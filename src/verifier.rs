//! Verificador da árvore verificada lida de um arquivo `.s`.
//!
//! Um `.s` pode ter sido editado à mão, e depois dele nada mais confere o programa. A
//! tradução para bytecode e o interpretador partem do princípio de que a árvore respeita
//! as regras da análise semântica; este verificador confere isso antes deles. Ele não infere tipos nem
//! resolve nomes: só confere se os tipos e vínculos já anotados são coerentes.

use crate::ast::{BinaryOperator, UnaryOperator};
use crate::checked::{
    Callee, CheckedBlock, CheckedCall, CheckedExpression, CheckedExpressionKind, CheckedFunction,
    CheckedProgram, CheckedStatement, CheckedStatementKind, Type, block_terminates,
};
use crate::diagnostic::{Diagnostic, error};
use crate::token::Position;

// pink-nav: start
// pink-resume: Ponto de entrada: confere a main, cada função e o fluxo de retorno de uma árvore lida de um .s.
// pink-keywords: verify, verificador, árvore inconsistente, main, função, fluxo
/// Confere a árvore. O erro aponta a posição do nó incoerente no código-fonte de origem.
pub fn verify(program: &CheckedProgram) -> Result<(), Diagnostic> {
    let main = program
        .functions
        .get(program.main)
        .filter(|main| main.name == "main")
        .ok_or_else(|| inconsistent("não há uma função 'main'", Position { line: 1, column: 1 }))?;
    if !main.parameters.is_empty() || main.result.is_some() {
        return Err(inconsistent(
            "'main' não pode ter parâmetros nem resultado",
            main.position,
        ));
    }
    for function in &program.functions {
        let mut verifier = Verifier {
            program,
            function,
            declared: vec![false; function.variables.len()],
            loop_depth: 0,
        };
        for &parameter in &function.parameters {
            let variable = function.variables.get(parameter).filter(|v| v.parameter);
            let Some(variable) = variable else {
                return Err(inconsistent("parâmetro sem declaração", function.position));
            };
            if verifier.declared[parameter] || variable.mutable {
                return Err(inconsistent(
                    "parâmetro repetido ou mutável",
                    variable.position,
                ));
            }
            verifier.declared[parameter] = true;
        }
        verifier.block(&function.body)?;
        if function.result.is_some() && !block_terminates(&function.body) {
            return Err(inconsistent(
                format!(
                    "a função '{}' pode chegar ao fim sem devolver o resultado",
                    function.name
                ),
                function.position,
            ));
        }
    }
    Ok(())
}

fn inconsistent(message: impl Into<String>, position: Position) -> Diagnostic {
    error(
        format!("árvore inconsistente: {}", message.into()),
        position,
    )
}

struct Verifier<'p> {
    program: &'p CheckedProgram,
    function: &'p CheckedFunction,
    /// As variáveis em escopo no ponto atual.
    declared: Vec<bool>,
    loop_depth: usize,
}
// pink-nav: end

impl Verifier<'_> {
    // pink-nav: start
    // pink-resume: Instruções: escopo das variáveis, mutabilidade, condições bool, give, break e continue.
    // pink-keywords: instrução, escopo, variável, declaração, mutabilidade, give, break, continue
    fn variable(&self, index: usize, position: Position) -> Result<Type, Diagnostic> {
        match self.function.variables.get(index) {
            Some(variable) if self.declared[index] => Ok(variable.ty),
            _ => Err(inconsistent(
                format!("a variável #{index} é usada fora do seu escopo"),
                position,
            )),
        }
    }

    fn block(&mut self, block: &CheckedBlock) -> Result<(), Diagnostic> {
        let mut local = Vec::new();
        let result = block
            .statements
            .iter()
            .try_for_each(|statement| self.statement(statement, &mut local));
        // As variáveis do bloco saem do escopo, mesmo se houve erro.
        for index in local {
            self.declared[index] = false;
        }
        result
    }

    fn statement(
        &mut self,
        statement: &CheckedStatement,
        local: &mut Vec<usize>,
    ) -> Result<(), Diagnostic> {
        let position = statement.position;
        match &statement.kind {
            CheckedStatementKind::Let { variable, value } => {
                let declared = self.function.variables.get(*variable);
                let Some(declared) = declared.filter(|v| !v.parameter) else {
                    return Err(inconsistent("let de uma variável inexistente", position));
                };
                if self.declared[*variable] {
                    return Err(inconsistent(
                        format!("a variável #{variable} é declarada duas vezes"),
                        position,
                    ));
                }
                self.expect(value, declared.ty)?;
                self.declared[*variable] = true;
                local.push(*variable);
            }
            CheckedStatementKind::Assign { variable, value } => {
                let ty = self.variable(*variable, position)?;
                if !self.function.variables[*variable].mutable {
                    return Err(inconsistent("atribuição a uma variável imutável", position));
                }
                self.expect(value, ty)?;
            }
            CheckedStatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.expect(condition, Type::Bool)?;
                self.block(then_block)?;
                if let Some(else_block) = else_block {
                    self.block(else_block)?;
                }
            }
            CheckedStatementKind::While { condition, body } => {
                self.expect(condition, Type::Bool)?;
                self.loop_body(body)?;
            }
            CheckedStatementKind::Loop { body } => self.loop_body(body)?,
            CheckedStatementKind::Break | CheckedStatementKind::Continue => {
                if self.loop_depth == 0 {
                    return Err(inconsistent("break ou continue fora de um laço", position));
                }
            }
            CheckedStatementKind::Give { value } => match (self.function.result, value) {
                (Some(result), Some(value)) => self.expect(value, result)?,
                (None, None) => {}
                _ => {
                    return Err(inconsistent(
                        "give incompatível com o resultado da função",
                        position,
                    ));
                }
            },
            CheckedStatementKind::Call(call) => {
                self.call(call, None, position)?;
            }
        }
        Ok(())
    }

    fn loop_body(&mut self, body: &CheckedBlock) -> Result<(), Diagnostic> {
        self.loop_depth += 1;
        let result = self.block(body);
        self.loop_depth -= 1;
        result
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Expressões e chamadas: o tipo anotado de cada nó precisa seguir a tabela de operadores e as assinaturas.
    // pink-keywords: expressão, tipo, operador, literal, faixa, chamada, argumento, morph, say
    fn expect(&self, expression: &CheckedExpression, ty: Type) -> Result<(), Diagnostic> {
        self.expression(expression)?;
        if expression.ty != ty {
            return Err(inconsistent(
                format!("esperava {ty}, a expressão é {}", expression.ty),
                expression.position,
            ));
        }
        Ok(())
    }

    /// Confere a expressão e se o tipo anotado nela é o que as regras dão.
    fn expression(&self, expression: &CheckedExpression) -> Result<(), Diagnostic> {
        use BinaryOperator::*;
        let position = expression.position;
        let ty = expression.ty;
        let coherent = match &expression.kind {
            CheckedExpressionKind::Integer(value) => {
                matches!(ty, Type::Int(int) if int.contains(*value))
            }
            CheckedExpressionKind::Bool(_) => ty == Type::Bool,
            CheckedExpressionKind::Text(_) => ty == Type::Text,
            CheckedExpressionKind::Variable(index) => self.variable(*index, position)? == ty,
            CheckedExpressionKind::Call(call) => self.call(call, Some(ty), position)? == Some(ty),
            CheckedExpressionKind::Unary { operator, operand } => {
                self.expression(operand)?;
                match operator {
                    UnaryOperator::Not => operand.ty == Type::Bool && ty == Type::Bool,
                    UnaryOperator::Negate => {
                        operand.ty == ty && matches!(ty, Type::Int(int) if int.signed())
                    }
                }
            }
            CheckedExpressionKind::Binary {
                operator,
                left,
                right,
            } => {
                self.expression(left)?;
                self.expression(right)?;
                let same = left.ty == right.ty;
                let integers = same && matches!(left.ty, Type::Int(_));
                match operator {
                    And | Or => left.ty == Type::Bool && right.ty == Type::Bool && ty == Type::Bool,
                    Equal | NotEqual => same && ty == Type::Bool,
                    Less | LessEqual | Greater | GreaterEqual => integers && ty == Type::Bool,
                    Add | Subtract | Multiply | Divide | Remainder => integers && ty == left.ty,
                }
            }
        };
        if !coherent {
            return Err(inconsistent(
                format!("o tipo {ty} não é coerente com esta expressão"),
                position,
            ));
        }
        Ok(())
    }

    /// Confere a chamada e devolve o tipo do resultado. `ty` é o tipo da expressão de
    /// chamada, se ela for uma expressão (é o destino de `morph`).
    fn call(
        &self,
        call: &CheckedCall,
        ty: Option<Type>,
        position: Position,
    ) -> Result<Option<Type>, Diagnostic> {
        for argument in &call.arguments {
            self.expression(argument)?;
        }
        match call.callee {
            Callee::Function(index) => {
                let Some(function) = self.program.functions.get(index) else {
                    return Err(inconsistent("chamada a uma função inexistente", position));
                };
                let parameters: Option<Vec<Type>> = function
                    .parameters
                    .iter()
                    .map(|&parameter| function.variables.get(parameter).map(|v| v.ty))
                    .collect();
                let arguments: Vec<Type> = call.arguments.iter().map(|a| a.ty).collect();
                if parameters != Some(arguments) {
                    return Err(inconsistent(
                        format!("argumentos incompatíveis com '{}'", function.name),
                        position,
                    ));
                }
                Ok(function.result)
            }
            Callee::Say => {
                if call.arguments.len() != 1 {
                    return Err(inconsistent(
                        "'say' recebe exatamente 1 argumento",
                        position,
                    ));
                }
                Ok(None)
            }
            Callee::Morph => {
                let valid = matches!(call.arguments.as_slice(), [argument] if matches!(argument.ty, Type::Int(_)))
                    && matches!(ty, Some(Type::Int(_)));
                if !valid {
                    return Err(inconsistent(
                        "'morph' precisa de um argumento inteiro e de um destino inteiro",
                        position,
                    ));
                }
                Ok(ty)
            }
        }
    }
    // pink-nav: end
}
