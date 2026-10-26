//! Árvore verificada: o resultado da análise semântica.
//!
//! Tem a mesma forma do programa, mas com os tipos resolvidos (`int` já é `i64`), cada
//! uso de variável ligado à sua declaração e cada chamada ligada à função chamada. É a
//! entrada do futuro interpretador.

use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::artifact::{
    Format, FormatError, TreeLine, TreeReader, format_error, header, identifier, literal,
    parse_position, split_word, tree_line,
};
use crate::ast::{BinaryOperator, UnaryOperator};
use crate::token::{Position, TokenKind, quote_text};

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
// pink-resume: Fluxo de retorno: se um bloco nunca chega ao fim e se um break sai do laço que o contém.
// pink-keywords: fluxo, give, break, loop, terminar, retorno
/// Verdadeiro quando o bloco nunca chega ao fim: em todos os caminhos, ele devolve um
/// valor ou fica preso num `loop` sem `break` que saia dele. Não chegar ao fim não é o
/// mesmo que devolver: `loop` com só `continue` nunca devolve, mas também nunca termina.
pub fn block_terminates(block: &CheckedBlock) -> bool {
    block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            CheckedStatementKind::Give { .. } => true,
            CheckedStatementKind::If {
                then_block,
                else_block: Some(else_block),
                ..
            } => block_terminates(then_block) && block_terminates(else_block),
            CheckedStatementKind::Loop { body } => !breaks_out(body),
            _ => false,
        })
}

/// Verdadeiro quando o bloco tem um `break` que sai do laço que o contém (um `break`
/// dentro de um laço interno pertence a esse laço).
pub fn breaks_out(block: &CheckedBlock) -> bool {
    block
        .statements
        .iter()
        .any(|statement| match &statement.kind {
            CheckedStatementKind::Break => true,
            CheckedStatementKind::If {
                then_block,
                else_block,
                ..
            } => breaks_out(then_block) || else_block.as_ref().is_some_and(breaks_out),
            _ => false,
        })
}
// pink-nav: end

// pink-nav: start
// pink-resume: Grava a árvore verificada no formato .s (saída de --emit=semantic), com tipos e variáveis numeradas.
// pink-keywords: emit, render, formatação, saída, semantic, tipos, vínculos, arquivo .s
/// O arquivo `.s`: uma linha por nó, como no `.a`, com o tipo de cada expressão. Cada
/// variável aparece como `nome#n`, em que `n` identifica a declaração dentro da função;
/// a declaração (`Parâmetro` ou `Let`) traz também o tipo e a posição do nome.
pub fn render(program: &CheckedProgram, source: &str) -> String {
    let mut output = header(Format::Semantic, source);
    output.push_str("#POSIÇÃO NÓ\n");
    let mut renderer = Renderer {
        program,
        variables: &[],
        output,
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
        tree_line(&mut self.output, position, depth, text);
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
                let text = format!(
                    "Let {mutable}{}@{}:{}: {}",
                    self.variable(*variable),
                    declared.position.line,
                    declared.position.column,
                    declared.ty
                );
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
                let text = format!("Texto {}: {ty}", quote_text(value));
                self.line(position, depth, &text);
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

// pink-nav: start
// pink-resume: Lê um arquivo .s de volta para a árvore verificada: funções por nome, variáveis numeradas e tipos anotados.
// pink-keywords: read, leitura, arquivo .s, árvore verificada, variável, tipo, chamada
/// Lê um arquivo `.s` (sem conferir o cabeçalho, que `artifact::detect` já leu). Confere a
/// forma e a numeração das variáveis; a coerência dos tipos e do fluxo fica para o
/// verificador (`verifier::verify`), que precisa rodar antes do interpretador.
pub fn read(content: &str) -> Result<CheckedProgram, FormatError> {
    let mut reader = TreeReader::new(content)?;
    // As chamadas apontam para as funções pelo nome; primeiro, os nomes de todas.
    let mut functions_by_name = HashMap::new();
    for line in reader.lines().iter().filter(|line| line.depth == 0) {
        let text = split_word(line.text).1;
        let name = text.split_once(" -> ").map_or(text, |(name, _)| name);
        if functions_by_name
            .insert(name.to_string(), functions_by_name.len())
            .is_some()
        {
            return Err(format_error(line.line, format!("função '{name}' repetida")));
        }
    }
    let mut functions = Vec::new();
    while let Some(line) = reader.child(0)? {
        let function = FunctionReader {
            reader: &mut reader,
            functions: &functions_by_name,
            variables: Vec::new(),
            references: Vec::new(),
        }
        .function(line)?;
        functions.push(function);
    }
    let main = *functions_by_name
        .get("main")
        .ok_or_else(|| format_error(1, "o programa não tem uma função 'main'"))?;
    Ok(CheckedProgram { functions, main })
}

fn parse_type(text: &str, line: usize) -> Result<Type, FormatError> {
    Type::from_name(text).ok_or_else(|| format_error(line, format!("tipo desconhecido '{text}'")))
}

/// `nome#n`: o nome e o número da variável.
fn variable_reference(text: &str, line: usize) -> Result<(&str, usize), FormatError> {
    let invalid = || format_error(line, format!("esperava nome#número, encontrei '{text}'"));
    let (name, index) = text.split_once('#').ok_or_else(invalid)?;
    let index = index.parse().map_err(|_| invalid())?;
    Ok((identifier(name, line)?, index))
}

struct FunctionReader<'r, 'a> {
    reader: &'r mut TreeReader<'a>,
    functions: &'r HashMap<String, usize>,
    /// As declarações, pelo número; todas precisam aparecer.
    variables: Vec<Option<Variable>>,
    /// Usos de variáveis (número, nome, linha), conferidos com as declarações no fim.
    references: Vec<(usize, String, usize)>,
}

impl<'a> FunctionReader<'_, 'a> {
    fn function(mut self, line: TreeLine<'a>) -> Result<CheckedFunction, FormatError> {
        let text = expect_word(&line, "Função")?;
        let (name, result) = match text.split_once(" -> ") {
            Some((name, result)) => (name, Some(parse_type(result, line.line)?)),
            None => (text, None),
        };
        let name = identifier(name, line.line)?.to_string();
        let mut parameters = Vec::new();
        while self
            .reader
            .peek_child(1)
            .is_some_and(|child| split_word(child.text).0 == "Parâmetro")
        {
            let child = self.reader.child(1)?.expect("peek_child confirmou a linha");
            let text = expect_word(&child, "Parâmetro")?;
            let (reference, ty) = text
                .split_once(": ")
                .ok_or_else(|| format_error(child.line, "esperava 'Parâmetro nome#n: tipo'"))?;
            let ty = parse_type(ty, child.line)?;
            let index = self.declare(reference, ty, false, true, child.position, child.line)?;
            parameters.push(index);
        }
        let body = self.reader.expect_child(1, "o bloco da função", &line)?;
        let body = self.block(body, 1)?;
        self.reader.end(1)?;

        let mut variables = Vec::new();
        for (index, variable) in self.variables.into_iter().enumerate() {
            variables.push(variable.ok_or_else(|| {
                format_error(
                    line.line,
                    format!("a variável #{index} de '{name}' não é declarada"),
                )
            })?);
        }
        for (index, reference, at) in self.references {
            let declared = variables.get(index).ok_or_else(|| {
                format_error(at, format!("a variável #{index} não existe em '{name}'"))
            })?;
            if declared.name != reference {
                return Err(format_error(
                    at,
                    format!(
                        "a variável #{index} se chama '{}', não '{reference}'",
                        declared.name
                    ),
                ));
            }
        }
        Ok(CheckedFunction {
            name,
            parameters,
            result,
            variables,
            body,
            position: line.position,
        })
    }

    fn declare(
        &mut self,
        reference: &str,
        ty: Type,
        mutable: bool,
        parameter: bool,
        position: Position,
        line: usize,
    ) -> Result<usize, FormatError> {
        let (name, index) = variable_reference(reference, line)?;
        if index >= self.variables.len() {
            self.variables.resize(index + 1, None);
        }
        if self.variables[index].is_some() {
            return Err(format_error(
                line,
                format!("a variável #{index} é declarada mais de uma vez"),
            ));
        }
        self.variables[index] = Some(Variable {
            name: name.to_string(),
            ty,
            mutable,
            parameter,
            position,
        });
        Ok(index)
    }

    fn reference(&mut self, text: &str, line: usize) -> Result<usize, FormatError> {
        let (name, index) = variable_reference(text, line)?;
        self.references.push((index, name.to_string(), line));
        Ok(index)
    }

    fn block(&mut self, line: TreeLine<'a>, depth: usize) -> Result<CheckedBlock, FormatError> {
        expect_word(&line, "Bloco")?;
        let mut statements = Vec::new();
        while let Some(child) = self.reader.child(depth + 1)? {
            statements.push(self.statement(child, depth + 1)?);
        }
        if statements.is_empty() {
            return Err(format_error(line.line, "bloco vazio"));
        }
        Ok(CheckedBlock {
            statements,
            position: line.position,
        })
    }

    fn expect_block(
        &mut self,
        parent: &TreeLine<'a>,
        depth: usize,
        what: &str,
    ) -> Result<CheckedBlock, FormatError> {
        let line = self.reader.expect_child(depth, what, parent)?;
        self.block(line, depth)
    }

    fn statement(
        &mut self,
        line: TreeLine<'a>,
        depth: usize,
    ) -> Result<CheckedStatement, FormatError> {
        let child = depth + 1;
        let (word, rest) = split_word(line.text);
        let kind = match word {
            "Let" => {
                let (mutable, rest) = match rest.strip_prefix("mut ") {
                    Some(rest) => (true, rest),
                    None => (false, rest),
                };
                let invalid =
                    || format_error(line.line, "esperava 'Let nome#n@linha:coluna: tipo'");
                let (declaration, ty) = rest.split_once(": ").ok_or_else(invalid)?;
                let (reference, position) = declaration.split_once('@').ok_or_else(invalid)?;
                let position = parse_position(position, line.line)?;
                let ty = parse_type(ty, line.line)?;
                let variable = self.declare(reference, ty, mutable, false, position, line.line)?;
                let value = self.expect_expression(&line, child)?;
                CheckedStatementKind::Let { variable, value }
            }
            "Atribuição" => {
                let variable = self.reference(rest, line.line)?;
                let value = self.expect_expression(&line, child)?;
                CheckedStatementKind::Assign { variable, value }
            }
            "If" => {
                let condition = self.expect_expression(&line, child)?;
                let then_block = self.expect_block(&line, child, "o bloco do if")?;
                let else_block = match self.reader.peek_child(child) {
                    Some(next) if next.text == "Else" => {
                        let else_line = self
                            .reader
                            .child(child)?
                            .expect("peek_child confirmou a linha");
                        let block = self.expect_block(&else_line, child + 1, "o bloco do else")?;
                        self.reader.end(child + 1)?;
                        Some(block)
                    }
                    _ => None,
                };
                CheckedStatementKind::If {
                    condition,
                    then_block,
                    else_block,
                }
            }
            "While" => {
                let condition = self.expect_expression(&line, child)?;
                let body = self.expect_block(&line, child, "o bloco do while")?;
                CheckedStatementKind::While { condition, body }
            }
            "Loop" => CheckedStatementKind::Loop {
                body: self.expect_block(&line, child, "o bloco do loop")?,
            },
            "Break" if rest.is_empty() => CheckedStatementKind::Break,
            "Continue" if rest.is_empty() => CheckedStatementKind::Continue,
            "Give" if rest.is_empty() => {
                let value = match self.reader.child(child)? {
                    Some(value) => Some(self.expression(value, child)?),
                    None => None,
                };
                CheckedStatementKind::Give { value }
            }
            "Chamada" => {
                // Numa chamada solta, um resultado anotado (`: tipo`) é descartado.
                let callee = rest.split_once(": ").map_or(rest, |(callee, _)| callee);
                CheckedStatementKind::Call(self.call(&line, callee, child)?)
            }
            _ => {
                return Err(format_error(
                    line.line,
                    format!("instrução desconhecida '{}'", line.text),
                ));
            }
        };
        self.reader.end(child)?;
        Ok(CheckedStatement {
            kind,
            position: line.position,
        })
    }

    fn expect_expression(
        &mut self,
        parent: &TreeLine<'a>,
        depth: usize,
    ) -> Result<CheckedExpression, FormatError> {
        let line = self.reader.expect_child(depth, "uma expressão", parent)?;
        self.expression(line, depth)
    }

    fn expression(
        &mut self,
        line: TreeLine<'a>,
        depth: usize,
    ) -> Result<CheckedExpression, FormatError> {
        let child = depth + 1;
        let invalid = || format_error(line.line, format!("expressão inválida '{}'", line.text));
        if let Some(conversion) = line.text.strip_prefix("Conversão morph: ") {
            let (from, to) = conversion.split_once(" -> ").ok_or_else(invalid)?;
            let (from, ty) = (parse_type(from, line.line)?, parse_type(to, line.line)?);
            let argument = self.expect_expression(&line, child)?;
            if argument.ty != from {
                return Err(format_error(
                    line.line,
                    format!(
                        "a conversão diz partir de {from}, mas o argumento é {}",
                        argument.ty
                    ),
                ));
            }
            self.reader.end(child)?;
            let call = CheckedCall {
                callee: Callee::Morph,
                arguments: vec![argument],
            };
            return Ok(CheckedExpression {
                kind: CheckedExpressionKind::Call(call),
                ty,
                position: line.position,
            });
        }
        let (head, ty) = line.text.rsplit_once(": ").ok_or_else(|| {
            format_error(
                line.line,
                format!("falta o tipo da expressão em '{}'", line.text),
            )
        })?;
        let ty = parse_type(ty, line.line)?;
        let (word, rest) = split_word(head);
        let kind = match word {
            "Inteiro" => CheckedExpressionKind::Integer(rest.parse().map_err(|_| invalid())?),
            "Lógico" => match rest {
                "true" => CheckedExpressionKind::Bool(true),
                "false" => CheckedExpressionKind::Bool(false),
                _ => return Err(invalid()),
            },
            "Texto" => match literal(rest, line.line)? {
                TokenKind::Text(value) => CheckedExpressionKind::Text(Rc::from(value.as_str())),
                _ => return Err(invalid()),
            },
            "Variável" => CheckedExpressionKind::Variable(self.reference(rest, line.line)?),
            "Chamada" => CheckedExpressionKind::Call(self.call(&line, rest, child)?),
            "Unária" => CheckedExpressionKind::Unary {
                operator: UnaryOperator::from_spelling(rest).ok_or_else(invalid)?,
                operand: Box::new(self.expect_expression(&line, child)?),
            },
            "Binária" => CheckedExpressionKind::Binary {
                operator: BinaryOperator::from_spelling(rest).ok_or_else(invalid)?,
                left: Box::new(self.expect_expression(&line, child)?),
                right: Box::new(self.expect_expression(&line, child)?),
            },
            _ => return Err(invalid()),
        };
        self.reader.end(child)?;
        Ok(CheckedExpression {
            kind,
            ty,
            position: line.position,
        })
    }

    /// `Chamada nome` ou `Chamada say (predefinida)`, com os argumentos como filhos.
    fn call(
        &mut self,
        line: &TreeLine<'a>,
        callee: &str,
        depth: usize,
    ) -> Result<CheckedCall, FormatError> {
        let callee =
            match callee {
                "say (predefinida)" => Callee::Say,
                name => Callee::Function(*self.functions.get(name).ok_or_else(|| {
                    format_error(line.line, format!("a função '{name}' não existe"))
                })?),
            };
        let mut arguments = Vec::new();
        while let Some(argument) = self.reader.child(depth)? {
            arguments.push(self.expression(argument, depth)?);
        }
        Ok(CheckedCall { callee, arguments })
    }
}

fn expect_word<'a>(line: &TreeLine<'a>, word: &str) -> Result<&'a str, FormatError> {
    match split_word(line.text) {
        (found, rest) if found == word => Ok(rest),
        _ => Err(format_error(
            line.line,
            format!("esperava '{word}', encontrei '{}'", line.text),
        )),
    }
}
// pink-nav: end
