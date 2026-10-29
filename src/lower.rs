//! Tradução da árvore verificada para bytecode.
//!
//! Cada função vira uma lista de instruções de pilha. O controle de fluxo (`if`, `while`,
//! `loop`, `break`, `continue` e o curto-circuito de `and` e `or`) vira saltos. Quando um
//! bloco termina, por qualquer saída, as variáveis declaradas nele são esvaziadas com
//! `clear`, como no interpretador. O código que vem depois de um `give`, `break` ou
//! `continue` no mesmo bloco nunca executa e não é traduzido.

use std::rc::Rc;

use crate::ast::{BinaryOperator, UnaryOperator};
use crate::bytecode::{BytecodeFunction, BytecodeProgram, Instruction, Operation};
use crate::checked::{
    Callee, CheckedBlock, CheckedCall, CheckedExpression, CheckedExpressionKind, CheckedFunction,
    CheckedProgram, CheckedStatement, CheckedStatementKind, IntType, Type, breaks_out,
};
use crate::token::Position;

// pink-nav: start
// pink-resume: Ponto de entrada: traduz cada função e acrescenta o give final das funções sem resultado.
// pink-keywords: lower, tradução, bytecode, função, give final
pub fn lower(program: &CheckedProgram) -> BytecodeProgram {
    let functions = program
        .functions
        .iter()
        .map(|function| {
            let mut lowering = Lowering {
                program,
                code: Vec::new(),
                scopes: Vec::new(),
                loops: Vec::new(),
            };
            // A análise semântica garante que uma função com resultado nunca chega ao fim
            // do corpo; uma sem resultado que chega devolve ali.
            if lowering.block(&function.body) {
                lowering.emit(Operation::Give, function.position);
            }
            lowered(function, lowering.code)
        })
        .collect();
    BytecodeProgram {
        functions,
        main: program.main,
    }
}

fn lowered(function: &CheckedFunction, code: Vec<Instruction>) -> BytecodeFunction {
    BytecodeFunction {
        name: function.name.clone(),
        parameters: function.parameters.clone(),
        result: function.result,
        variables: function.variables.clone(),
        code,
        position: function.position,
        name_position: function.name_position,
    }
}

struct Lowering<'p> {
    program: &'p CheckedProgram,
    code: Vec<Instruction>,
    /// As variáveis declaradas até agora em cada bloco aberto, do mais externo ao mais
    /// interno, com a posição do `let` de cada uma.
    scopes: Vec<Vec<(usize, Position)>>,
    loops: Vec<Loop>,
}

/// Um laço aberto: para onde vai o `continue`, quantos blocos já estavam abertos antes do
/// corpo e os saltos dos `break`, que só ganham destino quando o laço termina.
struct Loop {
    start: usize,
    scopes: usize,
    breaks: Vec<usize>,
}

fn int(ty: Type) -> IntType {
    match ty {
        Type::Int(int) => int,
        other => unreachable!("tipo verificado como inteiro: {other}"),
    }
}
// pink-nav: end

impl Lowering<'_> {
    // pink-nav: start
    // pink-resume: Instruções e saltos: emitir, apontar um salto para o ponto atual e esvaziar as variáveis dos blocos que terminam.
    // pink-keywords: emit, salto, destino, patch, clear, escopo, bloco
    fn emit(&mut self, operation: Operation, position: Position) -> usize {
        self.code.push(Instruction {
            operation,
            position,
        });
        self.code.len() - 1
    }

    /// Faz o salto da instrução `jump` apontar para a próxima instrução a ser emitida.
    fn land(&mut self, jump: usize) {
        let here = self.code.len();
        match &mut self.code[jump].operation {
            Operation::Jump(target) | Operation::JumpFalse(target) => *target = here,
            other => unreachable!("só saltos ganham destino: {other:?}"),
        }
    }

    /// Esvazia as variáveis dos blocos abertos a partir do bloco `from`, do mais interno ao
    /// mais externo.
    fn clear_scopes(&mut self, from: usize) {
        let variables: Vec<(usize, Position)> = self.scopes[from..]
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev().copied())
            .collect();
        for (variable, position) in variables {
            self.emit(Operation::Clear(variable), position);
        }
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Blocos e instruções: let, atribuição, if/else, while, loop, break, continue, give e chamadas soltas viram instruções e saltos.
    // pink-keywords: bloco, instrução, let, store, if, else, while, loop, break, continue, give, pop, salto
    /// Traduz o bloco e devolve se a execução pode sair pelo fim dele. Nesse caso, as
    /// variáveis do bloco são esvaziadas ali; as saídas por `break` e `continue` esvaziam
    /// as suas antes de saltar, e `give` descarta o quadro inteiro.
    fn block(&mut self, block: &CheckedBlock) -> bool {
        self.scopes.push(Vec::new());
        let mut reaches_end = true;
        for statement in &block.statements {
            if !self.statement(statement) {
                reaches_end = false;
                break;
            }
        }
        if reaches_end {
            self.clear_scopes(self.scopes.len() - 1);
        }
        self.scopes.pop();
        reaches_end
    }

    /// Traduz a instrução e devolve se a execução pode seguir para a próxima.
    fn statement(&mut self, statement: &CheckedStatement) -> bool {
        let position = statement.position;
        match &statement.kind {
            CheckedStatementKind::Let { variable, value } => {
                self.expression(value);
                self.emit(Operation::Store(*variable), position);
                let scope = self.scopes.last_mut().expect("sempre há um bloco aberto");
                scope.push((*variable, position));
                true
            }
            CheckedStatementKind::Assign { variable, value } => {
                self.expression(value);
                self.emit(Operation::Store(*variable), position);
                true
            }
            CheckedStatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.expression(condition);
                let to_else = self.emit(Operation::JumpFalse(0), position);
                let then_continues = self.block(then_block);
                let Some(else_block) = else_block else {
                    self.land(to_else);
                    return true;
                };
                let to_end = then_continues.then(|| self.emit(Operation::Jump(0), position));
                self.land(to_else);
                let else_continues = self.block(else_block);
                if let Some(to_end) = to_end {
                    self.land(to_end);
                }
                then_continues || else_continues
            }
            CheckedStatementKind::While { condition, body } => {
                let start = self.code.len();
                self.expression(condition);
                let exit = self.emit(Operation::JumpFalse(0), position);
                self.loop_body(body, start, position);
                self.land(exit);
                self.land_breaks();
                true
            }
            CheckedStatementKind::Loop { body } => {
                let start = self.code.len();
                self.loop_body(body, start, position);
                self.land_breaks();
                breaks_out(body)
            }
            CheckedStatementKind::Break | CheckedStatementKind::Continue => {
                let innermost = self.loops.last().expect("break e continue só em laços");
                let (from, start) = (innermost.scopes, innermost.start);
                self.clear_scopes(from);
                if matches!(statement.kind, CheckedStatementKind::Break) {
                    let jump = self.emit(Operation::Jump(0), position);
                    self.loops.last_mut().unwrap().breaks.push(jump);
                } else {
                    self.emit(Operation::Jump(start), position);
                }
                false
            }
            CheckedStatementKind::Give { value } => {
                if let Some(value) = value {
                    self.expression(value);
                }
                self.emit(Operation::Give, position);
                false
            }
            CheckedStatementKind::Call(call) => {
                let returns = self.call(call, None, position);
                if returns {
                    self.emit(Operation::Pop, position);
                }
                true
            }
        }
    }

    /// O corpo de um laço: se ele pode chegar ao fim, volta para o início.
    fn loop_body(&mut self, body: &CheckedBlock, start: usize, position: Position) {
        self.loops.push(Loop {
            start,
            scopes: self.scopes.len(),
            breaks: Vec::new(),
        });
        if self.block(body) {
            self.emit(Operation::Jump(start), position);
        }
    }

    /// Os `break` do laço que acabou de ser traduzido saltam para depois dele.
    fn land_breaks(&mut self) {
        let finished = self.loops.pop().expect("um laço aberto");
        for jump in finished.breaks {
            self.land(jump);
        }
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Expressões: operandos da esquerda para a direita, operadores tipados, curto-circuito com saltos e chamadas.
    // pink-keywords: expressão, operador, curto-circuito, and, or, constante, load, chamada, say, morph
    fn expression(&mut self, expression: &CheckedExpression) {
        let position = expression.position;
        match &expression.kind {
            CheckedExpressionKind::Integer(value) => {
                self.emit(Operation::Integer(*value, int(expression.ty)), position);
            }
            CheckedExpressionKind::Bool(value) => {
                self.emit(Operation::Bool(*value), position);
            }
            CheckedExpressionKind::Text(value) => {
                self.emit(Operation::Text(Rc::clone(value)), position);
            }
            CheckedExpressionKind::Variable(variable) => {
                self.emit(Operation::Load(*variable), position);
            }
            CheckedExpressionKind::Call(call) => {
                self.call(call, Some(expression.ty), position);
            }
            CheckedExpressionKind::Unary { operator, operand } => {
                self.expression(operand);
                let operation = match operator {
                    UnaryOperator::Not => Operation::Not,
                    UnaryOperator::Negate => Operation::Negate(int(expression.ty)),
                };
                self.emit(operation, position);
            }
            CheckedExpressionKind::Binary {
                operator,
                left,
                right,
            } => match operator {
                // a and b: se a for falso, o resultado é falso sem avaliar b.
                BinaryOperator::And => {
                    self.expression(left);
                    let short = self.emit(Operation::JumpFalse(0), position);
                    self.expression(right);
                    let end = self.emit(Operation::Jump(0), position);
                    self.land(short);
                    self.emit(Operation::Bool(false), position);
                    self.land(end);
                }
                // a or b: se a for verdadeiro, o resultado é verdadeiro sem avaliar b.
                BinaryOperator::Or => {
                    self.expression(left);
                    let evaluate_right = self.emit(Operation::JumpFalse(0), position);
                    self.emit(Operation::Bool(true), position);
                    let end = self.emit(Operation::Jump(0), position);
                    self.land(evaluate_right);
                    self.expression(right);
                    self.land(end);
                }
                BinaryOperator::Equal | BinaryOperator::NotEqual => {
                    self.expression(left);
                    self.expression(right);
                    self.emit(Operation::Equality(*operator, left.ty), position);
                }
                _ => {
                    self.expression(left);
                    self.expression(right);
                    self.emit(Operation::Binary(*operator, int(left.ty)), position);
                }
            },
        }
    }

    /// Traduz a chamada e devolve se ela deixa um resultado na pilha. `ty` é o tipo do
    /// resultado quando a chamada é uma expressão (é o destino de `morph`).
    fn call(&mut self, call: &CheckedCall, ty: Option<Type>, position: Position) -> bool {
        for argument in &call.arguments {
            self.expression(argument);
        }
        match call.callee {
            Callee::Function(index) => {
                self.emit(Operation::Call(index), position);
                self.program.functions[index].result.is_some()
            }
            Callee::Say => {
                self.emit(Operation::Say(call.arguments[0].ty), position);
                false
            }
            Callee::Morph => {
                let from = int(call.arguments[0].ty);
                let to = int(ty.expect("morph sempre é uma expressão"));
                self.emit(Operation::Morph(from, to), position);
                true
            }
        }
    }
    // pink-nav: end
}
