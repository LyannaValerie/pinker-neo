//! Análise semântica: verifica nomes, tipos e regras de contexto da AST e produz a
//! árvore verificada (`checked`).
//!
//! Trabalha em duas passagens: primeiro coleta as assinaturas de todas as funções (o
//! que permite chamar uma função declarada depois e a recursão), depois verifica o corpo
//! de cada uma. Para no primeiro erro. As regras estão em `docs/semantica.md`.

use std::collections::HashMap;
use std::rc::Rc;

use crate::ast::{self, BinaryOperator, ElseBranch, ExpressionKind, StatementKind, UnaryOperator};
use crate::checked::{
    Callee, CheckedBlock, CheckedCall, CheckedExpression, CheckedExpressionKind, CheckedFunction,
    CheckedProgram, CheckedStatement, CheckedStatementKind, IntType, Type, Variable, arithmetic,
    block_terminates,
};
use crate::diagnostic::{Diagnostic, error};
use crate::token::Position;

/// Funções predefinidas. Não são palavras reservadas, mas não podem ser redeclaradas.
const PREDEFINED: &[&str] = &["say", "morph"];

// pink-nav: start
// pink-resume: Ponto de entrada: coleta as assinaturas, valida a main e verifica cada função.
// pink-keywords: check, análise semântica, assinatura, main, duplicata, tipo desconhecido
pub fn check(program: &ast::Program) -> Result<CheckedProgram, Diagnostic> {
    let signatures = collect_signatures(program)?;
    let main = check_main(program, &signatures)?;
    let functions = program
        .functions
        .iter()
        .map(|function| check_function(&signatures, function))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CheckedProgram { functions, main })
}

struct Signature {
    parameters: Vec<Type>,
    result: Option<Type>,
    position: Position,
}

/// As assinaturas, na mesma ordem das funções do programa.
struct Signatures {
    list: Vec<Signature>,
    by_name: HashMap<String, usize>,
}

fn collect_signatures(program: &ast::Program) -> Result<Signatures, Diagnostic> {
    let mut signatures = Signatures {
        list: Vec::new(),
        by_name: HashMap::new(),
    };
    for function in &program.functions {
        let name = &function.name;
        reject_predefined(name)?;
        if let Some(&previous) = signatures.by_name.get(&name.text) {
            let previous = signatures.list[previous].position;
            return Err(error(
                format!(
                    "a função '{}' já foi declarada em {}:{}",
                    name.text, previous.line, previous.column
                ),
                name.position,
            ));
        }
        let parameters = function
            .parameters
            .iter()
            .map(|parameter| resolve_type(&parameter.type_name))
            .collect::<Result<Vec<_>, _>>()?;
        let result = function.result.as_ref().map(resolve_type).transpose()?;
        signatures
            .by_name
            .insert(name.text.clone(), signatures.list.len());
        signatures.list.push(Signature {
            parameters,
            result,
            position: name.position,
        });
    }
    Ok(signatures)
}

fn check_main(program: &ast::Program, signatures: &Signatures) -> Result<usize, Diagnostic> {
    let Some(&index) = signatures.by_name.get("main") else {
        return Err(error(
            "o programa não tem uma função 'main'",
            Position { line: 1, column: 1 },
        ));
    };
    let main = &program.functions[index];
    if let Some(parameter) = main.parameters.first() {
        return Err(error(
            "'main' não recebe parâmetros",
            parameter.name.position,
        ));
    }
    if let Some(result) = &main.result {
        return Err(error(
            format!("'main' não devolve resultado; remova '-> {}'", result.text),
            result.position,
        ));
    }
    Ok(index)
}

fn resolve_type(name: &ast::Name) -> Result<Type, Diagnostic> {
    Type::from_name(&name.text).ok_or_else(|| {
        error(
            format!(
                "tipo desconhecido '{}'; os tipos disponíveis são int, i8, i16, i32, i64, u8, u16, u32, u64, bool e text",
                name.text
            ),
            name.position,
        )
    })
}

fn reject_predefined(name: &ast::Name) -> Result<(), Diagnostic> {
    if PREDEFINED.contains(&name.text.as_str()) {
        return Err(error(
            format!(
                "'{}' é uma função predefinida e não pode ser redeclarada",
                name.text
            ),
            name.position,
        ));
    }
    Ok(())
}
// pink-nav: end

// pink-nav: start
// pink-resume: Verificação de uma função: escopos, variáveis, instruções e o fluxo que exige um give no fim.
// pink-keywords: função, escopo, variável, sombreamento, mutabilidade, instrução, give, break, fluxo
fn check_function(
    signatures: &Signatures,
    function: &ast::Function,
) -> Result<CheckedFunction, Diagnostic> {
    let signature = &signatures.list[signatures.by_name[&function.name.text]];
    let mut checker = FunctionChecker {
        signatures,
        result: signature.result,
        variables: Vec::new(),
        scopes: vec![HashMap::new()],
        loop_depth: 0,
        unevaluated: 0,
    };
    let mut parameters = Vec::new();
    for (parameter, ty) in function.parameters.iter().zip(&signature.parameters) {
        parameters.push(checker.declare(&parameter.name, *ty, false, true)?);
    }
    // Os parâmetros e o nível de cima do corpo dividem o mesmo escopo.
    let body = checker.statements(&function.body)?;
    if let Some(result) = signature.result
        && !block_terminates(&body)
    {
        return Err(error(
            format!(
                "a função '{}' pode chegar ao fim sem devolver um valor do tipo {result}",
                function.name.text
            ),
            function.name.position,
        ));
    }
    Ok(CheckedFunction {
        name: function.name.text.clone(),
        parameters,
        result: signature.result,
        variables: checker.variables,
        body,
        position: function.position,
        name_position: function.name.position,
    })
}

struct FunctionChecker<'s> {
    signatures: &'s Signatures,
    result: Option<Type>,
    variables: Vec<Variable>,
    /// Do escopo mais externo ao mais interno: nome → índice em `variables`.
    scopes: Vec<HashMap<String, usize>>,
    loop_depth: usize,
    /// Maior que zero dentro de código que comprovadamente não executa: o operando que o
    /// curto-circuito pula ou o ramo de uma condição constante. Nesse caso, nomes, tipos e
    /// literais são verificados, mas falhas de avaliação constante não.
    unevaluated: usize,
}

impl FunctionChecker<'_> {
    fn declare(
        &mut self,
        name: &ast::Name,
        ty: Type,
        mutable: bool,
        parameter: bool,
    ) -> Result<usize, Diagnostic> {
        reject_predefined(name)?;
        let scope = self.scopes.last_mut().expect("sempre há um escopo aberto");
        if let Some(&previous) = scope.get(&name.text) {
            let previous = self.variables[previous].position;
            return Err(error(
                format!(
                    "'{}' já foi declarado neste escopo, em {}:{}",
                    name.text, previous.line, previous.column
                ),
                name.position,
            ));
        }
        let index = self.variables.len();
        scope.insert(name.text.clone(), index);
        self.variables.push(Variable {
            name: name.text.clone(),
            ty,
            mutable,
            parameter,
            position: name.position,
        });
        Ok(index)
    }

    fn lookup(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    /// Um bloco com escopo próprio.
    fn block(&mut self, block: &ast::Block) -> Result<CheckedBlock, Diagnostic> {
        self.scopes.push(HashMap::new());
        let checked = self.statements(block);
        self.scopes.pop();
        checked
    }

    /// As instruções de um bloco, no escopo atual.
    fn statements(&mut self, block: &ast::Block) -> Result<CheckedBlock, Diagnostic> {
        let statements = block
            .statements
            .iter()
            .map(|statement| self.statement(statement))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CheckedBlock {
            statements,
            position: block.position,
        })
    }

    fn statement(&mut self, statement: &ast::Statement) -> Result<CheckedStatement, Diagnostic> {
        let position = statement.position;
        let kind = match &statement.kind {
            StatementKind::Let {
                mutable,
                name,
                type_name,
                value,
            } => {
                // O valor é verificado antes da declaração: em `let x = x`, o `x` da
                // direita é outro (de um escopo externo) ou não existe.
                let value = match type_name {
                    Some(type_name) => self.expect(value, resolve_type(type_name)?)?,
                    None => self.expression(value, None)?,
                };
                let variable = self.declare(name, value.ty, *mutable, false)?;
                CheckedStatementKind::Let { variable, value }
            }
            StatementKind::Assign { target, value } => {
                let Some(variable) = self.lookup(&target.text) else {
                    return Err(error(
                        format!("o nome '{}' não foi declarado", target.text),
                        target.position,
                    ));
                };
                let declared = &self.variables[variable];
                if !declared.mutable {
                    let message = if declared.parameter {
                        format!("o parâmetro '{}' é imutável", target.text)
                    } else {
                        format!(
                            "a variável '{}' não é mutável; declare-a com 'let mut'",
                            target.text
                        )
                    };
                    return Err(error(message, target.position));
                }
                let ty = declared.ty;
                let value = self.expect(value, ty)?;
                CheckedStatementKind::Assign { variable, value }
            }
            StatementKind::If {
                condition,
                then_block,
                else_branch,
            } => {
                let condition = self.expect(condition, Type::Bool)?;
                // Com condição constante, um dos ramos nunca executa.
                let decided = match constant(&condition) {
                    Some(Constant::Bool(value)) => Some(value),
                    _ => None,
                };
                let then_block =
                    self.unevaluated_if(decided == Some(false), |s| s.block(then_block))?;
                let else_block = match else_branch {
                    None => None,
                    Some(else_branch) => Some(self.unevaluated_if(decided == Some(true), |s| {
                        match &else_branch.branch {
                            ElseBranch::Block(block) => s.block(block),
                            ElseBranch::If(inner) => {
                                s.scopes.push(HashMap::new());
                                let inner = s.statement(inner);
                                s.scopes.pop();
                                Ok(CheckedBlock {
                                    statements: vec![inner?],
                                    position: else_branch.position,
                                })
                            }
                        }
                    })?),
                };
                CheckedStatementKind::If {
                    condition,
                    then_block,
                    else_block,
                }
            }
            StatementKind::While { condition, body } => {
                let condition = self.expect(condition, Type::Bool)?;
                let never = constant(&condition) == Some(Constant::Bool(false));
                let body = self.unevaluated_if(never, |s| s.loop_body(body))?;
                CheckedStatementKind::While { condition, body }
            }
            StatementKind::Loop { body } => CheckedStatementKind::Loop {
                body: self.loop_body(body)?,
            },
            StatementKind::Break | StatementKind::Continue => {
                if self.loop_depth == 0 {
                    let word = if statement.kind == StatementKind::Break {
                        "break"
                    } else {
                        "continue"
                    };
                    return Err(error(format!("'{word}' fora de um laço"), position));
                }
                if statement.kind == StatementKind::Break {
                    CheckedStatementKind::Break
                } else {
                    CheckedStatementKind::Continue
                }
            }
            StatementKind::Give { value } => {
                let value = match (self.result, value) {
                    (Some(result), Some(value)) => Some(self.expect(value, result)?),
                    (Some(result), None) => {
                        return Err(error(
                            format!("'give' sem valor numa função que devolve {result}"),
                            position,
                        ));
                    }
                    (None, Some(value)) => {
                        return Err(error(
                            "esta função não devolve resultado; use 'give' sem valor",
                            value.position,
                        ));
                    }
                    (None, None) => None,
                };
                CheckedStatementKind::Give { value }
            }
            StatementKind::Call(call) => CheckedStatementKind::Call(self.call(call, None)?.0),
        };
        Ok(CheckedStatement { kind, position })
    }

    /// Verifica algo que comprovadamente nunca executa quando `skip` é verdadeiro: nomes,
    /// tipos e literais continuam sendo verificados, mas falhas de avaliação constante não.
    fn unevaluated_if<T>(
        &mut self,
        skip: bool,
        check: impl FnOnce(&mut Self) -> Result<T, Diagnostic>,
    ) -> Result<T, Diagnostic> {
        self.unevaluated += usize::from(skip);
        let result = check(self);
        self.unevaluated -= usize::from(skip);
        result
    }

    fn loop_body(&mut self, body: &ast::Block) -> Result<CheckedBlock, Diagnostic> {
        self.loop_depth += 1;
        let body = self.block(body);
        self.loop_depth -= 1;
        body
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Verificação de expressões: tipos dos operadores, inferência dos literais e avaliação constante.
    // pink-keywords: expressão, operador, literal, inferência, faixa, overflow, divisão por zero, curto-circuito
    /// Verifica uma expressão e exige que ela tenha o tipo `ty`.
    fn expect(
        &mut self,
        expression: &ast::Expression,
        ty: Type,
    ) -> Result<CheckedExpression, Diagnostic> {
        let checked = self.expression(expression, Some(ty))?;
        if checked.ty != ty {
            return Err(error(
                format!("esperava um valor do tipo {ty}, encontrei {}", checked.ty),
                checked.position,
            ));
        }
        Ok(checked)
    }

    /// Verifica uma expressão. `expected` é o tipo que o contexto pede, se houver: dá o
    /// tipo aos literais inteiros e o destino de `morph`. Quem chama confere o resultado.
    fn expression(
        &mut self,
        expression: &ast::Expression,
        expected: Option<Type>,
    ) -> Result<CheckedExpression, Diagnostic> {
        let position = expression.position;
        let (kind, ty) = match &expression.kind {
            ExpressionKind::Integer { base, digits } => {
                let ty = integer_type(expected);
                let value = literal_value(base.radix(), digits, false, ty, position)?;
                (CheckedExpressionKind::Integer(value), Type::Int(ty))
            }
            ExpressionKind::Bool(value) => (CheckedExpressionKind::Bool(*value), Type::Bool),
            ExpressionKind::Text(value) => (
                CheckedExpressionKind::Text(Rc::from(value.as_str())),
                Type::Text,
            ),
            ExpressionKind::Name(name) => {
                let Some(variable) = self.lookup(name) else {
                    let is_function = self.signatures.by_name.contains_key(name)
                        || PREDEFINED.contains(&name.as_str());
                    let message = if is_function {
                        format!(
                            "'{name}' é uma função; funções só podem ser chamadas, como em '{name}(...)'"
                        )
                    } else {
                        format!("o nome '{name}' não foi declarado")
                    };
                    return Err(error(message, position));
                };
                (
                    CheckedExpressionKind::Variable(variable),
                    self.variables[variable].ty,
                )
            }
            ExpressionKind::Call(call) => {
                let (checked, result) = self.call(call, expected)?;
                let Some(ty) = result else {
                    return Err(error(
                        format!(
                            "'{}' não devolve resultado e não pode ser usada como valor",
                            call.callee.text
                        ),
                        position,
                    ));
                };
                (CheckedExpressionKind::Call(checked), ty)
            }
            ExpressionKind::Unary { operator, operand } => {
                return self.unary(*operator, operand, expected, position);
            }
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } => return self.binary(*operator, left, right, expected, position),
        };
        Ok(CheckedExpression { kind, ty, position })
    }

    fn unary(
        &mut self,
        operator: UnaryOperator,
        operand: &ast::Expression,
        expected: Option<Type>,
        position: Position,
    ) -> Result<CheckedExpression, Diagnostic> {
        if operator == UnaryOperator::Not {
            let operand = self.expect(operand, Type::Bool)?;
            return Ok(unary(operator, operand, Type::Bool, position));
        }
        // `-` aplicado direto a um literal forma um literal negativo. É o que permite
        // escrever o mínimo de i64, cuja magnitude sozinha não cabe em i64.
        if let ExpressionKind::Integer { base, digits } = &operand.kind {
            let ty = integer_type(expected);
            let value = literal_value(base.radix(), digits, true, ty, position)?;
            return Ok(CheckedExpression {
                kind: CheckedExpressionKind::Integer(value),
                ty: Type::Int(ty),
                position,
            });
        }
        let operand = self.expression(operand, expected)?;
        let ty = operand.ty;
        let Type::Int(int) = ty else {
            return Err(error(
                format!("o '-' prefixo exige um inteiro com sinal; encontrei {ty}"),
                position,
            ));
        };
        if !int.signed() {
            return Err(error(
                format!("o '-' prefixo exige um inteiro com sinal; encontrei {ty}"),
                position,
            ));
        }
        if self.unevaluated == 0
            && let Some(Constant::Int(value)) = constant(&operand)
            && !int.contains(-value)
        {
            return Err(error(
                format!("overflow: -({value}) não cabe em {ty}"),
                position,
            ));
        }
        Ok(unary(operator, operand, ty, position))
    }

    fn binary(
        &mut self,
        operator: BinaryOperator,
        left: &ast::Expression,
        right: &ast::Expression,
        expected: Option<Type>,
        position: Position,
    ) -> Result<CheckedExpression, Diagnostic> {
        use BinaryOperator::*;
        let symbol = operator.as_str();
        match operator {
            And | Or => {
                let left = self.expect(left, Type::Bool)?;
                // Se o operando da esquerda já decide o resultado, o da direita não é
                // avaliado: ele é verificado, mas não pode gerar falha de avaliação.
                let skipped = matches!(
                    (operator, constant(&left)),
                    (And, Some(Constant::Bool(false))) | (Or, Some(Constant::Bool(true)))
                );
                let right = self.unevaluated_if(skipped, |s| s.expect(right, Type::Bool))?;
                Ok(binary(operator, left, right, Type::Bool, position))
            }
            Equal | NotEqual => {
                let (left, right) = self.operands(left, right, None)?;
                same_type(symbol, &left, &right, position)?;
                Ok(binary(operator, left, right, Type::Bool, position))
            }
            Less | LessEqual | Greater | GreaterEqual => {
                let (left, right) = self.operands(left, right, None)?;
                same_type(symbol, &left, &right, position)?;
                integer_operands(symbol, left.ty, position)?;
                Ok(binary(operator, left, right, Type::Bool, position))
            }
            Add | Subtract | Multiply | Divide | Remainder => {
                let hint = expected.filter(|ty| matches!(ty, Type::Int(_)));
                let (left, right) = self.operands(left, right, hint)?;
                same_type(symbol, &left, &right, position)?;
                // `+` entre dois textos é a concatenação: produz um texto novo. Ela não
                // passa pela avaliação constante, que só conhece inteiros e lógicos, e
                // não tem falha que se possa apontar antes da execução.
                if operator == Add && left.ty == Type::Text {
                    return Ok(binary(operator, left, right, Type::Text, position));
                }
                let int = arithmetic_operands(operator, left.ty, position)?;
                if self.unevaluated == 0 {
                    self.constant_arithmetic(operator, &left, &right, int, position)?;
                }
                let ty = left.ty;
                Ok(binary(operator, left, right, ty, position))
            }
        }
    }

    /// Verifica os dois operandos. Um literal sem tipo recebe o tipo do outro operando:
    /// em `x + 1` com `x: u8`, o `1` é `u8`.
    fn operands(
        &mut self,
        left: &ast::Expression,
        right: &ast::Expression,
        hint: Option<Type>,
    ) -> Result<(CheckedExpression, CheckedExpression), Diagnostic> {
        if is_untyped(left) && !is_untyped(right) {
            let right = self.expression(right, hint)?;
            let left = self.expression(left, Some(right.ty))?;
            Ok((left, right))
        } else {
            let left = self.expression(left, hint)?;
            let right = self.expression(right, Some(left.ty))?;
            Ok((left, right))
        }
    }

    /// Divisão por zero e overflow detectáveis antes da execução.
    fn constant_arithmetic(
        &self,
        operator: BinaryOperator,
        left: &CheckedExpression,
        right: &CheckedExpression,
        int: IntType,
        position: Position,
    ) -> Result<(), Diagnostic> {
        let divides = matches!(operator, BinaryOperator::Divide | BinaryOperator::Remainder);
        if divides && constant(right) == Some(Constant::Int(0)) {
            return Err(error("divisão por zero", position));
        }
        let (Some(Constant::Int(a)), Some(Constant::Int(b))) = (constant(left), constant(right))
        else {
            return Ok(());
        };
        let symbol = operator.as_str();
        match arithmetic(operator, a, b) {
            Some(value) if int.contains(value) => Ok(()),
            Some(value) => Err(error(
                format!("overflow: {a} {symbol} {b} = {value} não cabe em {int}"),
                position,
            )),
            None => Err(error(
                format!("overflow: o resultado de {a} {symbol} {b} não cabe em {int}"),
                position,
            )),
        }
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Verificação de chamadas: funções do programa, say e morph com o tipo de destino vindo do contexto.
    // pink-keywords: chamada, argumento, say, morph, conversão, predefinida, aridade
    /// Verifica uma chamada e devolve o tipo do resultado (`None` se não houver).
    fn call(
        &mut self,
        call: &ast::Call,
        expected: Option<Type>,
    ) -> Result<(CheckedCall, Option<Type>), Diagnostic> {
        let name = &call.callee;
        match name.text.as_str() {
            "say" => {
                let argument = single_argument(call)?;
                let argument = self.expression(argument, None)?;
                let call = CheckedCall {
                    callee: Callee::Say,
                    arguments: vec![argument],
                };
                Ok((call, None))
            }
            "morph" => {
                let argument = single_argument(call)?;
                let Some(Type::Int(target)) = expected else {
                    return Err(error(
                        "'morph' precisa de um tipo de destino inteiro vindo do contexto, como em 'let x: i64 = morph(v)'",
                        name.position,
                    ));
                };
                let argument = self.expression(argument, None)?;
                if !matches!(argument.ty, Type::Int(_)) {
                    return Err(error(
                        format!(
                            "'morph' converte só entre inteiros; encontrei {}",
                            argument.ty
                        ),
                        argument.position,
                    ));
                }
                if self.unevaluated == 0
                    && let Some(Constant::Int(value)) = constant(&argument)
                    && !target.contains(value)
                {
                    return Err(error(
                        format!("conversão fora da faixa: {value} não cabe em {target}"),
                        name.position,
                    ));
                }
                let call = CheckedCall {
                    callee: Callee::Morph,
                    arguments: vec![argument],
                };
                Ok((call, Some(Type::Int(target))))
            }
            _ => {
                let signatures = self.signatures;
                let Some(&index) = signatures.by_name.get(&name.text) else {
                    let message = if self.lookup(&name.text).is_some() {
                        format!("'{}' é uma variável, não uma função", name.text)
                    } else {
                        format!("a função '{}' não foi declarada", name.text)
                    };
                    return Err(error(message, name.position));
                };
                let signature = &signatures.list[index];
                if call.arguments.len() != signature.parameters.len() {
                    return Err(error(
                        format!(
                            "'{}' recebe {}; recebeu {}",
                            name.text,
                            arguments(signature.parameters.len()),
                            call.arguments.len()
                        ),
                        name.position,
                    ));
                }
                let arguments = call
                    .arguments
                    .iter()
                    .zip(&signature.parameters)
                    .map(|(argument, ty)| self.expect(argument, *ty))
                    .collect::<Result<Vec<_>, _>>()?;
                let call = CheckedCall {
                    callee: Callee::Function(index),
                    arguments,
                };
                Ok((call, signature.result))
            }
        }
    }
    // pink-nav: end
}

// pink-nav: start
// pink-resume: Auxiliares sem estado: literais, constantes, tipos de operandos e argumentos.
// pink-keywords: literal, constante, operando, argumento, auxiliar
fn integer_type(expected: Option<Type>) -> IntType {
    match expected {
        Some(Type::Int(int)) => int,
        _ => IntType::I64,
    }
}

/// O valor de um literal inteiro no tipo `ty`, ou erro se não couber.
fn literal_value(
    radix: u32,
    digits: &str,
    negative: bool,
    ty: IntType,
    position: Position,
) -> Result<i128, Diagnostic> {
    let sign = if negative { "-" } else { "" };
    let magnitude = u128::from_str_radix(digits, radix)
        .ok()
        .and_then(|magnitude| i128::try_from(magnitude).ok());
    let Some(magnitude) = magnitude else {
        return Err(error(
            format!("o literal {sign}{digits} é grande demais para qualquer tipo inteiro"),
            position,
        ));
    };
    let value = if negative { -magnitude } else { magnitude };
    if !ty.contains(value) {
        return Err(error(
            format!(
                "o valor {value} não cabe em {ty} (faixa: {} a {})",
                ty.min(),
                ty.max()
            ),
            position,
        ));
    }
    Ok(value)
}

/// Um literal inteiro (ou uma conta só com literais) ainda sem tipo decidido.
fn is_untyped(expression: &ast::Expression) -> bool {
    match &expression.kind {
        ExpressionKind::Integer { .. } => true,
        ExpressionKind::Unary {
            operator: UnaryOperator::Negate,
            operand,
        } => is_untyped(operand),
        ExpressionKind::Binary {
            operator:
                BinaryOperator::Add
                | BinaryOperator::Subtract
                | BinaryOperator::Multiply
                | BinaryOperator::Divide
                | BinaryOperator::Remainder,
            left,
            right,
        } => is_untyped(left) && is_untyped(right),
        _ => false,
    }
}

fn same_type(
    symbol: &str,
    left: &CheckedExpression,
    right: &CheckedExpression,
    position: Position,
) -> Result<(), Diagnostic> {
    if left.ty != right.ty {
        return Err(error(
            format!(
                "o operador '{symbol}' exige operandos do mesmo tipo; encontrei {} e {}",
                left.ty, right.ty
            ),
            position,
        ));
    }
    Ok(())
}

/// Os operandos de `+ - * / %`, já do mesmo tipo: inteiros, ou textos no caso de `+`, que
/// quem chama trata antes.
fn arithmetic_operands(
    operator: BinaryOperator,
    ty: Type,
    position: Position,
) -> Result<IntType, Diagnostic> {
    match ty {
        Type::Int(int) => Ok(int),
        _ if operator == BinaryOperator::Add => Err(error(
            format!("o operador '+' exige inteiros ou textos; encontrei {ty}"),
            position,
        )),
        _ => integer_operands(operator.as_str(), ty, position),
    }
}

fn integer_operands(symbol: &str, ty: Type, position: Position) -> Result<IntType, Diagnostic> {
    match ty {
        Type::Int(int) => Ok(int),
        _ => Err(error(
            format!("o operador '{symbol}' exige inteiros; encontrei {ty}"),
            position,
        )),
    }
}

fn single_argument(call: &ast::Call) -> Result<&ast::Expression, Diagnostic> {
    match call.arguments.as_slice() {
        [argument] => Ok(argument),
        _ => Err(error(
            format!(
                "'{}' recebe exatamente 1 argumento; recebeu {}",
                call.callee.text,
                call.arguments.len()
            ),
            call.callee.position,
        )),
    }
}

fn arguments(count: usize) -> String {
    if count == 1 {
        "1 argumento".to_string()
    } else {
        format!("{count} argumentos")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Constant {
    Int(i128),
    Bool(bool),
}

/// O valor de uma expressão que só depende de literais, se ela tiver um. Respeita o
/// curto-circuito: em `false and x`, o resultado é `false` mesmo que `x` não seja constante.
fn constant(expression: &CheckedExpression) -> Option<Constant> {
    use BinaryOperator::*;
    match &expression.kind {
        CheckedExpressionKind::Integer(value) => Some(Constant::Int(*value)),
        CheckedExpressionKind::Bool(value) => Some(Constant::Bool(*value)),
        CheckedExpressionKind::Unary { operator, operand } => {
            match (operator, constant(operand)?) {
                (UnaryOperator::Negate, Constant::Int(value)) => Some(Constant::Int(-value)),
                (UnaryOperator::Not, Constant::Bool(value)) => Some(Constant::Bool(!value)),
                _ => None,
            }
        }
        CheckedExpressionKind::Binary {
            operator,
            left,
            right,
        } => match (operator, constant(left)?) {
            (And, Constant::Bool(false)) => Some(Constant::Bool(false)),
            (Or, Constant::Bool(true)) => Some(Constant::Bool(true)),
            (And | Or, _) => constant(right),
            (_, left) => match (left, constant(right)?) {
                (Constant::Int(a), Constant::Int(b)) => match operator {
                    Equal => Some(Constant::Bool(a == b)),
                    NotEqual => Some(Constant::Bool(a != b)),
                    Less => Some(Constant::Bool(a < b)),
                    LessEqual => Some(Constant::Bool(a <= b)),
                    Greater => Some(Constant::Bool(a > b)),
                    GreaterEqual => Some(Constant::Bool(a >= b)),
                    _ => {
                        let Type::Int(int) = expression.ty else {
                            return None;
                        };
                        arithmetic(*operator, a, b)
                            .filter(|value| int.contains(*value))
                            .map(Constant::Int)
                    }
                },
                (Constant::Bool(a), Constant::Bool(b)) => match operator {
                    Equal => Some(Constant::Bool(a == b)),
                    NotEqual => Some(Constant::Bool(a != b)),
                    _ => None,
                },
                _ => None,
            },
        },
        _ => None,
    }
}

fn unary(
    operator: UnaryOperator,
    operand: CheckedExpression,
    ty: Type,
    position: Position,
) -> CheckedExpression {
    CheckedExpression {
        kind: CheckedExpressionKind::Unary {
            operator,
            operand: Box::new(operand),
        },
        ty,
        position,
    }
}

fn binary(
    operator: BinaryOperator,
    left: CheckedExpression,
    right: CheckedExpression,
    ty: Type,
    position: Position,
) -> CheckedExpression {
    CheckedExpression {
        kind: CheckedExpressionKind::Binary {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        },
        ty,
        position,
    }
}

// pink-nav: end
