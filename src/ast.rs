//! AST: a árvore de sintaxe que o parser constrói a partir dos tokens.
//!
//! A árvore registra só a estrutura do programa. Tipos, nomes e regras de contexto
//! (como `break` fora de laço) são verificados pela análise semântica.

use crate::artifact::{
    Format, FormatError, TreeLine, TreeReader, format_error, header, identifier, literal,
    parse_position, split_field, split_word, tree_line,
};
use crate::token::{Base, Position, TokenKind, quote_text};

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

    pub fn from_spelling(spelling: &str) -> Option<UnaryOperator> {
        [UnaryOperator::Negate, UnaryOperator::Not]
            .into_iter()
            .find(|operator| operator.as_str() == spelling)
    }
}

impl BinaryOperator {
    const ALL: [BinaryOperator; 13] = [
        BinaryOperator::Or,
        BinaryOperator::And,
        BinaryOperator::Equal,
        BinaryOperator::NotEqual,
        BinaryOperator::Less,
        BinaryOperator::LessEqual,
        BinaryOperator::Greater,
        BinaryOperator::GreaterEqual,
        BinaryOperator::Add,
        BinaryOperator::Subtract,
        BinaryOperator::Multiply,
        BinaryOperator::Divide,
        BinaryOperator::Remainder,
    ];

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

    pub fn from_spelling(spelling: &str) -> Option<BinaryOperator> {
        Self::ALL
            .into_iter()
            .find(|operator| operator.as_str() == spelling)
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Grava a AST no formato .a (saída de --emit=ast): árvore indentada com a posição de cada nó e de cada nome.
// pink-keywords: emit, render, formatação, saída, ast, árvore, arquivo .a
/// O arquivo `.a`: o cabeçalho e uma linha por nó, com a posição e a descrição, e os
/// filhos indentados sob o pai. Nomes e tipos levam a própria posição em `@linha:coluna`.
pub fn render(program: &Program, source: &str) -> String {
    let mut output = header(Format::Ast, source);
    output.push_str("#POSIÇÃO NÓ\n");
    let mut renderer = Renderer { output };
    for function in &program.functions {
        renderer.function(function, 0);
    }
    renderer.output
}

/// `nome@linha:coluna`.
fn at(name: &Name) -> String {
    format!(
        "{}@{}:{}",
        name.text, name.position.line, name.position.column
    )
}

struct Renderer {
    output: String,
}

impl Renderer {
    fn line(&mut self, position: Position, depth: usize, text: &str) {
        tree_line(&mut self.output, position, depth, text);
    }

    fn function(&mut self, function: &Function, depth: usize) {
        let result = match &function.result {
            Some(type_name) => format!(" -> {}", at(type_name)),
            None => String::new(),
        };
        let text = format!("Função {}{result}", at(&function.name));
        self.line(function.position, depth, &text);
        for parameter in &function.parameters {
            let text = format!(
                "Parâmetro {}: {}",
                parameter.name.text,
                at(&parameter.type_name)
            );
            self.line(parameter.name.position, depth + 1, &text);
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
                    Some(type_name) => format!(": {}", at(type_name)),
                    None => String::new(),
                };
                self.line(
                    position,
                    depth,
                    &format!("Let {mutable}{}{type_name}", at(name)),
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
                let prefix = match base {
                    Base::Decimal => "",
                    Base::Hexadecimal => "0x",
                    Base::Binary => "0b",
                };
                let text = format!("Inteiro {prefix}{digits} (base {})", base.radix());
                self.line(position, depth, &text);
            }
            ExpressionKind::Text(value) => {
                self.line(position, depth, &format!("Texto {}", quote_text(value)));
            }
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

// pink-nav: start
// pink-resume: Lê um arquivo .a de volta para a AST, nó por nó, conferindo a forma de cada linha e os filhos esperados.
// pink-keywords: read, leitura, arquivo .a, ast, árvore, nó, filho
/// Lê um arquivo `.a` (sem conferir o cabeçalho, que `artifact::detect` já leu). Confere
/// só a forma: a análise semântica confere o resto, como faz com a saída do parser.
pub fn read(content: &str) -> Result<Program, FormatError> {
    let mut reader = TreeReader::new(content)?;
    let mut functions = Vec::new();
    while let Some(line) = reader.child(0)? {
        functions.push(read_function(&mut reader, line)?);
    }
    Ok(Program { functions })
}

/// O texto de um nó depois da palavra que o identifica, ou erro se for outro nó.
fn node<'a>(line: &TreeLine<'a>, word: &str) -> Result<&'a str, FormatError> {
    match split_word(line.text) {
        (found, rest) if found == word => Ok(rest),
        _ => Err(format_error(
            line.line,
            format!("esperava '{word}', encontrei '{}'", line.text),
        )),
    }
}

/// `nome@linha:coluna`.
fn named(text: &str, line: usize) -> Result<Name, FormatError> {
    let (name, position) = text.split_once('@').ok_or_else(|| {
        format_error(
            line,
            format!("esperava nome@linha:coluna, encontrei '{text}'"),
        )
    })?;
    Ok(Name {
        text: identifier(name, line)?.to_string(),
        position: parse_position(position, line)?,
    })
}

fn read_function(reader: &mut TreeReader, line: TreeLine) -> Result<Function, FormatError> {
    let text = node(&line, "Função")?;
    let (name, result) = match text.split_once(" -> ") {
        Some((name, result)) => (name, Some(result)),
        None => (text, None),
    };
    let name = named(name, line.line)?;
    let result = result.map(|result| named(result, line.line)).transpose()?;
    let mut parameters = Vec::new();
    while reader
        .peek_child(1)
        .is_some_and(|child| split_word(child.text).0 == "Parâmetro")
    {
        let child = reader.child(1)?.expect("peek_child confirmou a linha");
        let text = node(&child, "Parâmetro")?;
        let (name, type_name) = text.split_once(": ").ok_or_else(|| {
            format_error(child.line, "esperava 'Parâmetro nome: tipo@linha:coluna'")
        })?;
        parameters.push(Parameter {
            name: Name {
                text: identifier(name, child.line)?.to_string(),
                position: child.position,
            },
            type_name: named(type_name, child.line)?,
        });
    }
    let body = reader.expect_child(1, "o bloco da função", &line)?;
    let body = read_block(reader, body, 1)?;
    reader.end(1)?;
    Ok(Function {
        name,
        parameters,
        result,
        body,
        position: line.position,
    })
}

fn read_block(reader: &mut TreeReader, line: TreeLine, depth: usize) -> Result<Block, FormatError> {
    node(&line, "Bloco")?;
    let mut statements = Vec::new();
    while let Some(child) = reader.child(depth + 1)? {
        statements.push(read_statement(reader, child, depth + 1)?);
    }
    if statements.is_empty() {
        return Err(format_error(line.line, "bloco vazio"));
    }
    Ok(Block {
        statements,
        position: line.position,
    })
}

fn read_statement(
    reader: &mut TreeReader,
    line: TreeLine,
    depth: usize,
) -> Result<Statement, FormatError> {
    let child = depth + 1;
    let (word, rest) = split_word(line.text);
    let kind = match word {
        "Let" => {
            let (mutable, rest) = match rest.strip_prefix("mut ") {
                Some(rest) => (true, rest),
                None => (false, rest),
            };
            let (name, type_name) = match rest.split_once(": ") {
                Some((name, type_name)) => (name, Some(type_name)),
                None => (rest, None),
            };
            StatementKind::Let {
                mutable,
                name: named(name, line.line)?,
                type_name: type_name.map(|t| named(t, line.line)).transpose()?,
                value: expect_expression(reader, &line, child)?,
            }
        }
        "Atribuição" => StatementKind::Assign {
            target: Name {
                text: identifier(rest, line.line)?.to_string(),
                position: line.position,
            },
            value: expect_expression(reader, &line, child)?,
        },
        "If" => {
            let condition = expect_expression(reader, &line, child)?;
            let then_block = reader.expect_child(child, "o bloco do if", &line)?;
            let then_block = read_block(reader, then_block, child)?;
            let else_branch = match reader.peek_child(child) {
                Some(next) if next.text == "Else" => {
                    let else_line = reader.child(child)?.expect("peek_child confirmou a linha");
                    let inner =
                        reader.expect_child(child + 1, "o bloco ou o if do else", &else_line)?;
                    let branch = if split_word(inner.text).0 == "If" {
                        ElseBranch::If(Box::new(read_statement(reader, inner, child + 1)?))
                    } else {
                        ElseBranch::Block(read_block(reader, inner, child + 1)?)
                    };
                    reader.end(child + 1)?;
                    Some(Else {
                        branch,
                        position: else_line.position,
                    })
                }
                _ => None,
            };
            StatementKind::If {
                condition,
                then_block,
                else_branch,
            }
        }
        "While" => {
            let condition = expect_expression(reader, &line, child)?;
            let body = reader.expect_child(child, "o bloco do while", &line)?;
            StatementKind::While {
                condition,
                body: read_block(reader, body, child)?,
            }
        }
        "Loop" => {
            let body = reader.expect_child(child, "o bloco do loop", &line)?;
            StatementKind::Loop {
                body: read_block(reader, body, child)?,
            }
        }
        "Break" if rest.is_empty() => StatementKind::Break,
        "Continue" if rest.is_empty() => StatementKind::Continue,
        "Give" if rest.is_empty() => {
            let value = match reader.child(child)? {
                Some(value) => Some(read_expression(reader, value, child)?),
                None => None,
            };
            StatementKind::Give { value }
        }
        "Chamada" => StatementKind::Call(read_call(reader, &line, rest, child)?),
        _ => {
            return Err(format_error(
                line.line,
                format!("instrução desconhecida '{}'", line.text),
            ));
        }
    };
    reader.end(child)?;
    Ok(Statement {
        kind,
        position: line.position,
    })
}

fn expect_expression(
    reader: &mut TreeReader,
    parent: &TreeLine,
    depth: usize,
) -> Result<Expression, FormatError> {
    let line = reader.expect_child(depth, "uma expressão", parent)?;
    read_expression(reader, line, depth)
}

fn read_expression(
    reader: &mut TreeReader,
    line: TreeLine,
    depth: usize,
) -> Result<Expression, FormatError> {
    let child = depth + 1;
    let (word, rest) = split_word(line.text);
    let invalid = || format_error(line.line, format!("expressão inválida '{}'", line.text));
    let kind = match word {
        "Inteiro" => match literal(split_field(rest).0, line.line)? {
            TokenKind::Integer { base, digits } => ExpressionKind::Integer { base, digits },
            _ => return Err(invalid()),
        },
        "Texto" => match literal(rest, line.line)? {
            TokenKind::Text(value) => ExpressionKind::Text(value),
            _ => return Err(invalid()),
        },
        "Lógico" => match rest {
            "true" => ExpressionKind::Bool(true),
            "false" => ExpressionKind::Bool(false),
            _ => return Err(invalid()),
        },
        "Nome" => ExpressionKind::Name(identifier(rest, line.line)?.to_string()),
        "Chamada" => ExpressionKind::Call(read_call(reader, &line, rest, child)?),
        "Unária" => ExpressionKind::Unary {
            operator: UnaryOperator::from_spelling(rest).ok_or_else(invalid)?,
            operand: Box::new(expect_expression(reader, &line, child)?),
        },
        "Binária" => ExpressionKind::Binary {
            operator: BinaryOperator::from_spelling(rest).ok_or_else(invalid)?,
            left: Box::new(expect_expression(reader, &line, child)?),
            right: Box::new(expect_expression(reader, &line, child)?),
        },
        _ => return Err(invalid()),
    };
    reader.end(child)?;
    Ok(Expression {
        kind,
        position: line.position,
    })
}

/// Os argumentos de uma chamada são os filhos da linha `Chamada nome`.
fn read_call(
    reader: &mut TreeReader,
    line: &TreeLine,
    callee: &str,
    depth: usize,
) -> Result<Call, FormatError> {
    let callee = Name {
        text: identifier(callee, line.line)?.to_string(),
        position: line.position,
    };
    let mut arguments = Vec::new();
    while let Some(argument) = reader.child(depth)? {
        arguments.push(read_expression(reader, argument, depth)?);
    }
    Ok(Call { callee, arguments })
}
// pink-nav: end
