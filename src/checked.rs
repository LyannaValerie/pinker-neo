//! Árvore verificada: o resultado da análise semântica.
//!
//! Tem a mesma forma do programa, mas com os tipos resolvidos (`int` já é `i64`), cada
//! uso de variável ligado à sua declaração e cada chamada ligada à função chamada. É a
//! entrada do futuro interpretador.

use std::fmt;
use std::rc::Rc;

use crate::ast::{BinaryOperator, UnaryOperator};
use crate::token::Position;

// pink-nav: start
// pink-resume: Tipos da primeira versão (inteiros de largura explícita, bool e text, com faixas e nomes) e a aritmética exata.
// pink-keywords: tipo, inteiro, i64, u8, faixa, mínimo, máximo, bool, text, int, aritmética
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

impl IntType {
    pub fn name(self) -> &'static str {
        match self {
            IntType::I8 => "i8",
            IntType::I16 => "i16",
            IntType::I32 => "i32",
            IntType::I64 => "i64",
            IntType::U8 => "u8",
            IntType::U16 => "u16",
            IntType::U32 => "u32",
            IntType::U64 => "u64",
        }
    }

    pub fn signed(self) -> bool {
        matches!(
            self,
            IntType::I8 | IntType::I16 | IntType::I32 | IntType::I64
        )
    }

    pub fn bits(self) -> u32 {
        match self {
            IntType::I8 | IntType::U8 => 8,
            IntType::I16 | IntType::U16 => 16,
            IntType::I32 | IntType::U32 => 32,
            IntType::I64 | IntType::U64 => 64,
        }
    }

    pub fn min(self) -> i128 {
        if self.signed() {
            -(1i128 << (self.bits() - 1))
        } else {
            0
        }
    }

    pub fn max(self) -> i128 {
        if self.signed() {
            (1i128 << (self.bits() - 1)) - 1
        } else {
            (1i128 << self.bits()) - 1
        }
    }

    pub fn contains(self, value: i128) -> bool {
        (self.min()..=self.max()).contains(&value)
    }
}

/// Aritmética exata em i128, com divisão truncada em direção a zero e resto com o sinal
/// do dividendo. `None` para divisão por zero ou resultado fora até de i128.
pub fn arithmetic(operator: BinaryOperator, a: i128, b: i128) -> Option<i128> {
    match operator {
        BinaryOperator::Add => a.checked_add(b),
        BinaryOperator::Subtract => a.checked_sub(b),
        BinaryOperator::Multiply => a.checked_mul(b),
        BinaryOperator::Divide => a.checked_div(b),
        BinaryOperator::Remainder => a.checked_rem(b),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Int(IntType),
    Bool,
    Text,
}

impl Type {
    /// O tipo de um nome escrito no programa. `int` é apelido de `i64`.
    pub fn from_name(name: &str) -> Option<Type> {
        let int = |t| Some(Type::Int(t));
        match name {
            "int" | "i64" => int(IntType::I64),
            "i8" => int(IntType::I8),
            "i16" => int(IntType::I16),
            "i32" => int(IntType::I32),
            "u8" => int(IntType::U8),
            "u16" => int(IntType::U16),
            "u32" => int(IntType::U32),
            "u64" => int(IntType::U64),
            "bool" => Some(Type::Bool),
            "text" => Some(Type::Text),
            _ => None,
        }
    }
}

impl fmt::Display for IntType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Type::Int(t) => f.write_str(t.name()),
            Type::Bool => f.write_str("bool"),
            Type::Text => f.write_str("text"),
        }
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Nós da árvore verificada: funções, variáveis numeradas, instruções e expressões com tipo.
// pink-keywords: árvore verificada, nó, variável, função, chamada, callee, expressão tipada
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedProgram {
    pub functions: Vec<CheckedFunction>,
    /// Índice da função `main` em `functions`.
    pub main: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedFunction {
    pub name: String,
    /// Índices dos parâmetros em `variables`, na ordem da assinatura.
    pub parameters: Vec<usize>,
    pub result: Option<Type>,
    /// Todas as variáveis da função (parâmetros e locais); os usos apontam para cá.
    pub variables: Vec<Variable>,
    pub body: CheckedBlock,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
    pub parameter: bool,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedBlock {
    pub statements: Vec<CheckedStatement>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedStatement {
    pub kind: CheckedStatementKind,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedStatementKind {
    Let {
        variable: usize,
        value: CheckedExpression,
    },
    Assign {
        variable: usize,
        value: CheckedExpression,
    },
    If {
        condition: CheckedExpression,
        then_block: CheckedBlock,
        /// Um `else if` vira um bloco com um único `if`.
        else_block: Option<CheckedBlock>,
    },
    While {
        condition: CheckedExpression,
        body: CheckedBlock,
    },
    Loop {
        body: CheckedBlock,
    },
    Break,
    Continue,
    Give {
        value: Option<CheckedExpression>,
    },
    Call(CheckedCall),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCall {
    pub callee: Callee,
    pub arguments: Vec<CheckedExpression>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    /// Índice da função em `CheckedProgram::functions`.
    Function(usize),
    Say,
    /// Conversão para o tipo da expressão de chamada.
    Morph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedExpression {
    pub kind: CheckedExpressionKind,
    pub ty: Type,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedExpressionKind {
    /// Valor já dentro da faixa do tipo; `-` aplicado direto a um literal entra no valor.
    Integer(i128),
    Bool(bool),
    /// Literais de texto: o armazenamento é criado uma vez e compartilhado por todos os
    /// valores que vêm dele.
    Text(Rc<str>),
    /// Índice da variável em `CheckedFunction::variables`.
    Variable(usize),
    Call(CheckedCall),
    Unary {
        operator: UnaryOperator,
        operand: Box<CheckedExpression>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<CheckedExpression>,
        right: Box<CheckedExpression>,
    },
}
// pink-nav: end

// pink-nav: start
// pink-resume: Formata a árvore verificada, com tipos e variáveis numeradas, para a saída de --emit=semantic.
// pink-keywords: emit, render, formatação, saída, semantic, tipos, vínculos
/// Uma linha por nó, como em `--emit=ast`, com o tipo de cada expressão. Cada variável
/// aparece como `nome#n`, em que `n` identifica a declaração dentro da função.
pub fn render(program: &CheckedProgram) -> String {
    let mut renderer = Renderer {
        program,
        variables: &[],
        output: String::from("POSIÇÃO  NÓ\n"),
    };
    for function in &program.functions {
        renderer.function(function);
    }
    renderer.output
}

struct Renderer<'a> {
    program: &'a CheckedProgram,
    variables: &'a [Variable],
    output: String,
}

impl<'a> Renderer<'a> {
    fn line(&mut self, position: Position, depth: usize, text: &str) {
        let position = format!("{}:{}", position.line, position.column);
        let indent = "  ".repeat(depth);
        self.output
            .push_str(&format!("{position:<8} {indent}{text}\n"));
    }

    fn variable(&self, index: usize) -> String {
        format!("{}#{index}", self.variables[index].name)
    }

    fn function(&mut self, function: &'a CheckedFunction) {
        self.variables = &function.variables;
        let result = match function.result {
            Some(ty) => format!(" -> {ty}"),
            None => String::new(),
        };
        self.line(
            function.position,
            0,
            &format!("Função {}{result}", function.name),
        );
        for &index in &function.parameters {
            let variable = &function.variables[index];
            let text = format!("Parâmetro {}: {}", self.variable(index), variable.ty);
            self.line(variable.position, 1, &text);
        }
        self.block(&function.body, 1);
    }

    fn block(&mut self, block: &CheckedBlock, depth: usize) {
        self.line(block.position, depth, "Bloco");
        for statement in &block.statements {
            self.statement(statement, depth + 1);
        }
    }

    fn statement(&mut self, statement: &CheckedStatement, depth: usize) {
        let position = statement.position;
        match &statement.kind {
            CheckedStatementKind::Let { variable, value } => {
                let declared = &self.variables[*variable];
                let mutable = if declared.mutable { "mut " } else { "" };
                let text = format!("Let {mutable}{}: {}", self.variable(*variable), declared.ty);
                self.line(position, depth, &text);
                self.expression(value, depth + 1);
            }
            CheckedStatementKind::Assign { variable, value } => {
                let text = format!("Atribuição {}", self.variable(*variable));
                self.line(position, depth, &text);
                self.expression(value, depth + 1);
            }
            CheckedStatementKind::If {
                condition,
                then_block,
                else_block,
            } => {
                self.line(position, depth, "If");
                self.expression(condition, depth + 1);
                self.block(then_block, depth + 1);
                if let Some(else_block) = else_block {
                    self.line(else_block.position, depth + 1, "Else");
                    self.block(else_block, depth + 2);
                }
            }
            CheckedStatementKind::While { condition, body } => {
                self.line(position, depth, "While");
                self.expression(condition, depth + 1);
                self.block(body, depth + 1);
            }
            CheckedStatementKind::Loop { body } => {
                self.line(position, depth, "Loop");
                self.block(body, depth + 1);
            }
            CheckedStatementKind::Break => self.line(position, depth, "Break"),
            CheckedStatementKind::Continue => self.line(position, depth, "Continue"),
            CheckedStatementKind::Give { value } => {
                self.line(position, depth, "Give");
                if let Some(value) = value {
                    self.expression(value, depth + 1);
                }
            }
            CheckedStatementKind::Call(call) => self.call(call, None, position, depth),
        }
    }

    fn call(&mut self, call: &CheckedCall, ty: Option<Type>, position: Position, depth: usize) {
        let text = match (call.callee, ty) {
            (Callee::Function(index), Some(ty)) => {
                format!("Chamada {}: {ty}", self.program.functions[index].name)
            }
            (Callee::Function(index), None) => {
                format!("Chamada {}", self.program.functions[index].name)
            }
            (Callee::Say, _) => "Chamada say (predefinida)".to_string(),
            (Callee::Morph, ty) => {
                let from = &call.arguments[0].ty;
                let to = ty.expect("morph sempre tem tipo de destino");
                format!("Conversão morph: {from} -> {to}")
            }
        };
        self.line(position, depth, &text);
        for argument in &call.arguments {
            self.expression(argument, depth + 1);
        }
    }

    fn expression(&mut self, expression: &CheckedExpression, depth: usize) {
        let position = expression.position;
        let ty = expression.ty;
        match &expression.kind {
            CheckedExpressionKind::Integer(value) => {
                self.line(position, depth, &format!("Inteiro {value}: {ty}"));
            }
            CheckedExpressionKind::Bool(value) => {
                self.line(position, depth, &format!("Lógico {value}: {ty}"));
            }
            CheckedExpressionKind::Text(value) => {
                self.line(position, depth, &format!("Texto {value:?}: {ty}"));
            }
            CheckedExpressionKind::Variable(index) => {
                let text = format!("Variável {}: {ty}", self.variable(*index));
                self.line(position, depth, &text);
            }
            CheckedExpressionKind::Call(call) => self.call(call, Some(ty), position, depth),
            CheckedExpressionKind::Unary { operator, operand } => {
                let text = format!("Unária {}: {ty}", operator.as_str());
                self.line(position, depth, &text);
                self.expression(operand, depth + 1);
            }
            CheckedExpressionKind::Binary {
                operator,
                left,
                right,
            } => {
                let text = format!("Binária {}: {ty}", operator.as_str());
                self.line(position, depth, &text);
                self.expression(left, depth + 1);
                self.expression(right, depth + 1);
            }
        }
    }
}
// pink-nav: end
