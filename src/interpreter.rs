//! Interpretador de árvore: executa a árvore verificada diretamente.
//!
//! Não é o caminho padrão da CLI, que traduz a árvore para bytecode e a executa na
//! máquina virtual (`vm`). Fica como referência: os testes exigem que os dois deem a mesma
//! saída e o mesmo diagnóstico, e os dois usam o mesmo runtime (`runtime`).
//!
//! Percorre a árvore produzida pela análise semântica e usa os vínculos já resolvidos:
//! cada variável é uma posição no quadro da chamada, e cada chamada aponta para a função
//! certa. Não refaz resolução de nomes nem inferência de tipos. As regras de execução
//! estão em `docs/execucao.md`.

use std::io::Write;
use std::rc::Rc;

use crate::ast::{BinaryOperator, UnaryOperator};
use crate::checked::{
    Callee, CheckedBlock, CheckedCall, CheckedExpression, CheckedExpressionKind, CheckedProgram,
    CheckedStatement, CheckedStatementKind, Type,
};
use crate::diagnostic::Diagnostic;
use crate::runtime::{binary_values, call_limit, convert, integer_type, negate, output_error, say};
use crate::token::Position;

/// Os valores e o limite de chamadas moraram aqui antes do runtime; o caminho antigo
/// continua valendo.
pub use crate::runtime::{MAX_CALL_DEPTH, Value};

// pink-nav: start
// pink-resume: Ponto de entrada: executa a main; chamadas de função com quadro de variáveis e limite de profundidade.
// pink-keywords: run, executar, main, chamada, quadro, frame, profundidade, recursão
/// Executa o programa a partir da `main`, escrevendo a saída de `say` em `output`.
/// Devolve a falha de execução, se houver; a saída escrita antes dela é preservada.
pub fn run(program: &CheckedProgram, output: &mut dyn Write) -> Result<(), Diagnostic> {
    let mut interpreter = Interpreter {
        program,
        output,
        depth: 0,
    };
    let main = &program.functions[program.main];
    interpreter.call_function(program.main, Vec::new(), main.position)?;
    interpreter
        .output
        .flush()
        .map_err(|e| output_error(e, main.position))
}

struct Interpreter<'p> {
    program: &'p CheckedProgram,
    output: &'p mut dyn Write,
    depth: usize,
}

/// Uma posição para cada variável da função; vazia antes do `let` e depois do bloco.
type Frame = Vec<Option<Value>>;

/// Como uma instrução terminou: normalmente ou desviando o fluxo.
enum Flow {
    Normal,
    Break,
    Continue,
    Give(Option<Value>),
}

impl Interpreter<'_> {
    fn call_function(
        &mut self,
        index: usize,
        arguments: Vec<Value>,
        position: Position,
    ) -> Result<Option<Value>, Diagnostic> {
        if self.depth >= MAX_CALL_DEPTH {
            return Err(call_limit(position));
        }
        let function = &self.program.functions[index];
        let mut frame: Frame = vec![None; function.variables.len()];
        for (&parameter, argument) in function.parameters.iter().zip(arguments) {
            frame[parameter] = Some(argument);
        }
        self.depth += 1;
        let flow = self.block(&function.body, &mut frame);
        self.depth -= 1;
        match flow? {
            Flow::Give(value) => Ok(value),
            // A análise semântica garante que uma função com resultado não chega ao fim
            // e que break e continue só aparecem dentro de laços.
            Flow::Normal => Ok(None),
            Flow::Break | Flow::Continue => unreachable!("break ou continue fora de laço"),
        }
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Execução de blocos e instruções, com propagação de give, break e continue.
    // pink-keywords: bloco, instrução, let, atribuição, if, while, loop, break, continue, give, fluxo
    fn block(&mut self, block: &CheckedBlock, frame: &mut Frame) -> Result<Flow, Diagnostic> {
        let mut flow = Ok(Flow::Normal);
        for statement in &block.statements {
            flow = self.statement(statement, frame);
            if !matches!(flow, Ok(Flow::Normal)) {
                break;
            }
        }
        // As variáveis declaradas neste bloco deixam de existir: os valores são soltos.
        for statement in &block.statements {
            if let CheckedStatementKind::Let { variable, .. } = statement.kind {
                frame[variable] = None;
            }
        }
        flow
    }

    fn statement(
        &mut self,
        statement: &CheckedStatement,
        frame: &mut Frame,
    ) -> Result<Flow, Diagnostic> {
        match &statement.kind {
            CheckedStatementKind::Let { variable, value }
            | CheckedStatementKind::Assign { variable, value } => {
                let value = self.expression(value, frame)?;
                frame[*variable] = Some(value);
            }
            CheckedStatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                if self.condition(condition, frame)? {
                    return self.block(then_block, frame);
                }
                if let Some(else_block) = else_block {
                    return self.block(else_block, frame);
                }
            }
            CheckedStatementKind::While { condition, body } => {
                while self.condition(condition, frame)? {
                    match self.block(body, frame)? {
                        Flow::Break => break,
                        Flow::Normal | Flow::Continue => {}
                        give @ Flow::Give(_) => return Ok(give),
                    }
                }
            }
            CheckedStatementKind::Loop { body } => loop {
                match self.block(body, frame)? {
                    Flow::Break => break,
                    Flow::Normal | Flow::Continue => {}
                    give @ Flow::Give(_) => return Ok(give),
                }
            },
            CheckedStatementKind::Break => return Ok(Flow::Break),
            CheckedStatementKind::Continue => return Ok(Flow::Continue),
            CheckedStatementKind::Give { value } => {
                let value = match value {
                    Some(value) => Some(self.expression(value, frame)?),
                    None => None,
                };
                return Ok(Flow::Give(value));
            }
            CheckedStatementKind::Call(call) => {
                self.call(call, None, statement.position, frame)?;
            }
        }
        Ok(Flow::Normal)
    }

    fn condition(
        &mut self,
        condition: &CheckedExpression,
        frame: &mut Frame,
    ) -> Result<bool, Diagnostic> {
        match self.expression(condition, frame)? {
            Value::Bool(value) => Ok(value),
            other => unreachable!("condição verificada como bool: {other:?}"),
        }
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Avaliação de expressões da esquerda para a direita, com curto-circuito e aritmética verificada.
    // pink-keywords: expressão, avaliação, ordem, curto-circuito, overflow, divisão por zero, negação
    fn expression(
        &mut self,
        expression: &CheckedExpression,
        frame: &mut Frame,
    ) -> Result<Value, Diagnostic> {
        let position = expression.position;
        match &expression.kind {
            CheckedExpressionKind::Integer(value) => Ok(Value::Int(*value)),
            CheckedExpressionKind::Bool(value) => Ok(Value::Bool(*value)),
            CheckedExpressionKind::Text(value) => Ok(Value::Text(Rc::clone(value))),
            CheckedExpressionKind::Variable(variable) => Ok(frame[*variable]
                .clone()
                .expect("a análise semântica garante que a variável foi declarada")),
            CheckedExpressionKind::Call(call) => {
                let value = self.call(call, Some(expression.ty), position, frame)?;
                Ok(value.expect("a análise semântica garante que a função devolve resultado"))
            }
            CheckedExpressionKind::Unary { operator, operand } => {
                let operand = self.expression(operand, frame)?;
                match (operator, operand) {
                    (UnaryOperator::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
                    (UnaryOperator::Negate, Value::Int(value)) => {
                        negate(value, integer_type(expression.ty), position)
                    }
                    (_, other) => unreachable!("operando verificado: {other:?}"),
                }
            }
            CheckedExpressionKind::Binary {
                operator,
                left,
                right,
            } => self.binary(*operator, left, right, expression.ty, position, frame),
        }
    }

    fn binary(
        &mut self,
        operator: BinaryOperator,
        left: &CheckedExpression,
        right: &CheckedExpression,
        ty: Type,
        position: Position,
        frame: &mut Frame,
    ) -> Result<Value, Diagnostic> {
        use BinaryOperator::*;
        let left = self.expression(left, frame)?;
        // Curto-circuito: o operando da direita só é avaliado se for preciso.
        match (operator, &left) {
            (And, Value::Bool(false)) => return Ok(Value::Bool(false)),
            (Or, Value::Bool(true)) => return Ok(Value::Bool(true)),
            (And | Or, _) => return self.expression(right, frame),
            _ => {}
        }
        let right = self.expression(right, frame)?;
        binary_values(operator, left, right, ty, position)
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Chamadas: argumentos da esquerda para a direita, say escrevendo na saída e morph conferindo a faixa.
    // pink-keywords: chamada, argumento, say, saída, morph, conversão, faixa
    /// Executa uma chamada. `ty` é o tipo do resultado quando a chamada é uma expressão.
    fn call(
        &mut self,
        call: &CheckedCall,
        ty: Option<Type>,
        position: Position,
        frame: &mut Frame,
    ) -> Result<Option<Value>, Diagnostic> {
        let mut arguments = Vec::with_capacity(call.arguments.len());
        for argument in &call.arguments {
            arguments.push(self.expression(argument, frame)?);
        }
        match call.callee {
            Callee::Function(index) => self.call_function(index, arguments, position),
            Callee::Say => {
                say(self.output, &arguments[0], position)?;
                Ok(None)
            }
            Callee::Morph => {
                let Value::Int(value) = arguments[0] else {
                    unreachable!("morph verificado com argumento inteiro");
                };
                let target = integer_type(ty.expect("morph sempre é uma expressão"));
                Ok(Some(convert(value, target, position)?))
            }
        }
    }
    // pink-nav: end
}

// pink-nav: start
// pink-resume: Testes internos: o quadro fica vazio depois de um bloco em qualquer saída, e o give conserva o texto devolvido.
// pink-keywords: teste, quadro, frame, liberação, texto, give, break, continue, falha
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer, parser, semantic};

    fn checked(source: &str) -> CheckedProgram {
        let tokens = lexer::tokenize(source).unwrap();
        semantic::check(&parser::parse(&tokens).unwrap()).unwrap()
    }

    /// Executa o corpo da função `index` e devolve o fluxo e o quadro logo depois.
    fn run_body(program: &CheckedProgram, index: usize) -> (Result<Flow, Diagnostic>, Frame) {
        let mut output = Vec::new();
        let mut interpreter = Interpreter {
            program,
            output: &mut output,
            depth: 1,
        };
        let function = &program.functions[index];
        let mut frame: Frame = vec![None; function.variables.len()];
        let flow = interpreter.block(&function.body, &mut frame);
        (flow, frame)
    }

    #[test]
    fn quadro_fica_vazio_apos_continue_break_e_fim_normal() {
        let program = checked(
            "fun main()\n    let mut i = 0\n    while i < 3\n        let a = \"continue\"\n        i = i + 1\n        if i == 1\n            continue\n        let b = \"break\"\n        if i == 2\n            break\n    loop\n        let c = \"loop\"\n        break\n",
        );
        let (flow, frame) = run_body(&program, 0);
        assert!(matches!(flow, Ok(Flow::Normal)));
        assert!(frame.iter().all(Option::is_none), "{frame:?}");
    }

    #[test]
    fn give_conserva_o_texto_e_esvazia_o_quadro() {
        let program = checked(
            "fun nome() -> text\n    let t = \"devolvido\"\n    if true\n        let u = t\n        give u\n    give t\n\nfun main()\n    say(nome())\n",
        );
        let (flow, frame) = run_body(&program, 0);
        let Ok(Flow::Give(Some(Value::Text(text)))) = flow else {
            panic!("esperava give com texto");
        };
        assert_eq!(&*text, "devolvido");
        assert!(frame.iter().all(Option::is_none), "{frame:?}");
        // Sobram duas referências: a do literal na árvore e a do valor devolvido.
        assert_eq!(Rc::strong_count(&text), 2);
    }

    #[test]
    fn falha_tambem_esvazia_o_quadro() {
        let program = checked(
            "fun main()\n    let d = \"antes da falha\"\n    if true\n        let e = \"dentro\"\n        let zero = 0\n        say(1 / zero)\n",
        );
        let (flow, frame) = run_body(&program, 0);
        assert!(flow.is_err());
        assert!(frame.iter().all(Option::is_none), "{frame:?}");
    }
}
// pink-nav: end
