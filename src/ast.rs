//! AST: a árvore de sintaxe que o parser constrói a partir dos tokens.
//!
//! A árvore registra só a estrutura do programa. Tipos, nomes e regras de contexto
//! (como `break` fora de laço) são verificados pela análise semântica.

use crate::token::{Base, Position};

// pink-nav: start
// pink-resume: Nós da AST: programa, funções, blocos, instruções e expressões, cada um com sua posição.
// pink-keywords: ast, árvore, nó, função, instrução, expressão, operador
/// Um nome escrito no programa (de função, variável, parâmetro ou tipo) e onde aparece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    pub text: String,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: Name,
    pub parameters: Vec<Parameter>,
    /// O tipo depois de `->`; ausente quando a função não devolve resultado.
    pub result: Option<Name>,
    pub body: Block,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: Name,
    pub type_name: Name,
}

/// Um bloco indentado; a posição é a da primeira instrução.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub kind: StatementKind,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementKind {
    Let {
        mutable: bool,
        name: Name,
        type_name: Option<Name>,
        value: Expression,
    },
    Assign {
        target: Name,
        value: Expression,
    },
    If {
        condition: Expression,
        then_block: Block,
        else_branch: Option<Else>,
    },
    While {
        condition: Expression,
        body: Block,
    },
    Loop {
        body: Block,
    },
    Break,
    Continue,
    Give {
        value: Option<Expression>,
    },
    Call(Call),
}

/// O `else` de um `if`: outro `if` (encadeamento `else if`) ou um bloco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Else {
    pub branch: ElseBranch,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElseBranch {
    If(Box<Statement>),
    Block(Block),
}

/// Uma chamada por nome, como `say(x)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub callee: Name,
    pub arguments: Vec<Expression>,
}

/// A posição de uma expressão binária é a do operador; as demais começam no primeiro token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionKind {
    /// Inteiro como foi escrito; a conversão e a faixa ficam para a análise semântica.
    Integer {
        base: Base,
        digits: String,
    },
    Text(String),
    Bool(bool),
    Name(String),
    Call(Call),
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
}

impl UnaryOperator {
    pub fn as_str(self) -> &'static str {
        match self {
            UnaryOperator::Negate => "-",
            UnaryOperator::Not => "not",
        }
    }
}

impl BinaryOperator {
    pub fn as_str(self) -> &'static str {
        match self {
            BinaryOperator::Or => "or",
            BinaryOperator::And => "and",
            BinaryOperator::Equal => "==",
            BinaryOperator::NotEqual => "!=",
            BinaryOperator::Less => "<",
            BinaryOperator::LessEqual => "<=",
            BinaryOperator::Greater => ">",
            BinaryOperator::GreaterEqual => ">=",
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Divide => "/",
            BinaryOperator::Remainder => "%",
        }
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Formata a AST como árvore indentada, com a posição de cada nó, para a saída de --emit=ast.
// pink-keywords: emit, render, formatação, saída, ast, árvore
/// Uma linha por nó: posição e descrição, com os filhos indentados sob o pai.
pub fn render(program: &Program) -> String {
    let mut out = Renderer {
        output: String::from("POSIÇÃO  NÓ\n"),
    };
    for function in &program.functions {
        out.function(function, 0);
    }
    out.output
}

struct Renderer {
    output: String,
}

impl Renderer {
    fn line(&mut self, position: Position, depth: usize, text: &str) {
        let position = format!("{}:{}", position.line, position.column);
        let indent = "  ".repeat(depth);
        self.output
            .push_str(&format!("{position:<8} {indent}{text}\n"));
    }

    fn function(&mut self, function: &Function, depth: usize) {
        let result = match &function.result {
            Some(type_name) => format!(" -> {}", type_name.text),
            None => String::new(),
        };
        self.line(
            function.position,
            depth,
            &format!("Função {}{result}", function.name.text),
        );
        for parameter in &function.parameters {
            self.line(
                parameter.name.position,
                depth + 1,
                &format!(
                    "Parâmetro {}: {}",
                    parameter.name.text, parameter.type_name.text
                ),
            );
        }
        self.block(&function.body, depth + 1);
    }

    fn block(&mut self, block: &Block, depth: usize) {
        self.line(block.position, depth, "Bloco");
        for statement in &block.statements {
            self.statement(statement, depth + 1);
        }
    }

    fn statement(&mut self, statement: &Statement, depth: usize) {
        let position = statement.position;
        match &statement.kind {
            StatementKind::Let {
                mutable,
                name,
                type_name,
                value,
            } => {
                let mutable = if *mutable { "mut " } else { "" };
                let type_name = match type_name {
                    Some(type_name) => format!(": {}", type_name.text),
                    None => String::new(),
                };
                self.line(
                    position,
                    depth,
                    &format!("Let {mutable}{}{type_name}", name.text),
                );
                self.expression(value, depth + 1);
            }
            StatementKind::Assign { target, value } => {
                self.line(position, depth, &format!("Atribuição {}", target.text));
                self.expression(value, depth + 1);
            }
            StatementKind::If {
                condition,
                then_block,
                else_branch,
            } => {
                self.line(position, depth, "If");
                self.expression(condition, depth + 1);
                self.block(then_block, depth + 1);
                if let Some(else_branch) = else_branch {
                    self.line(else_branch.position, depth + 1, "Else");
                    match &else_branch.branch {
                        ElseBranch::If(statement) => self.statement(statement, depth + 2),
                        ElseBranch::Block(block) => self.block(block, depth + 2),
                    }
                }
            }
            StatementKind::While { condition, body } => {
                self.line(position, depth, "While");
                self.expression(condition, depth + 1);
                self.block(body, depth + 1);
            }
            StatementKind::Loop { body } => {
                self.line(position, depth, "Loop");
                self.block(body, depth + 1);
            }
            StatementKind::Break => self.line(position, depth, "Break"),
            StatementKind::Continue => self.line(position, depth, "Continue"),
            StatementKind::Give { value } => {
                self.line(position, depth, "Give");
                if let Some(value) = value {
                    self.expression(value, depth + 1);
                }
            }
            StatementKind::Call(call) => self.call(call, position, depth),
        }
    }

    fn call(&mut self, call: &Call, position: Position, depth: usize) {
        self.line(position, depth, &format!("Chamada {}", call.callee.text));
        for argument in &call.arguments {
            self.expression(argument, depth + 1);
        }
    }

    fn expression(&mut self, expression: &Expression, depth: usize) {
        let position = expression.position;
        match &expression.kind {
            ExpressionKind::Integer { base, digits } => {
                self.line(
                    position,
                    depth,
                    &format!("Inteiro {digits} (base {})", base.radix()),
                );
            }
            ExpressionKind::Text(value) => self.line(position, depth, &format!("Texto {value:?}")),
            ExpressionKind::Bool(value) => self.line(position, depth, &format!("Lógico {value}")),
            ExpressionKind::Name(name) => self.line(position, depth, &format!("Nome {name}")),
            ExpressionKind::Call(call) => self.call(call, position, depth),
            ExpressionKind::Unary { operator, operand } => {
                self.line(position, depth, &format!("Unária {}", operator.as_str()));
                self.expression(operand, depth + 1);
            }
            ExpressionKind::Binary {
                operator,
                left,
                right,
            } => {
                self.line(position, depth, &format!("Binária {}", operator.as_str()));
                self.expression(left, depth + 1);
                self.expression(right, depth + 1);
            }
        }
    }
}
// pink-nav: end
