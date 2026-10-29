//! Árvore verificada: o resultado da análise semântica.
//!
//! Tem a mesma forma do programa, mas com os tipos resolvidos (`int` já é `i64`), cada
//! uso de variável ligado à sua declaração e cada chamada ligada à função chamada. É a
//! entrada da tradução para bytecode (`lower`) e do interpretador de árvore.

use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::artifact::{
    Format, FormatError, TreeLine, TreeReader, content_lines, format_error, header, identifier,
    literal, parse_position, split_field, split_word, tree_line,
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
    /// Posição do `fun`, onde a função começa.
    pub position: Position,
    /// Posição do nome: é a declaração que a tabela de símbolos registra.
    pub name_position: Position,
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
// pink-resume: Grava a árvore verificada no formato .s (saída de --emit=semantic): a tabela de símbolos e a árvore com tipos.
// pink-keywords: emit, render, formatação, saída, semantic, tipos, vínculos, arquivo .s, tabela de símbolos
/// O arquivo `.s` tem três partes. Primeiro, a tabela de símbolos: `[funções]`, com a
/// assinatura de cada função, e uma seção `[variáveis <função>]` por função, com cada
/// variável pelo número. Depois, `[árvore]`: uma linha por nó, como no `.a`, com o tipo
/// de cada expressão. A árvore se refere às variáveis por `nome#n` e às funções pelo
/// nome, e consulta a tabela para saber o tipo, a mutabilidade e a posição de cada uma.
pub fn render(program: &CheckedProgram, source: &str) -> String {
    let mut output = header(Format::Semantic, source);
    let symbols: Vec<FunctionSymbols> = program
        .functions
        .iter()
        .map(|function| FunctionSymbols {
            name: &function.name,
            name_position: function.name_position,
            parameters: &function.parameters,
            result: function.result,
            variables: &function.variables,
        })
        .collect();
    render_symbols(&mut output, &symbols);
    output.push_str("\n[árvore]\n#POSIÇÃO NÓ\n");
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

/// O que a tabela de símbolos mostra de uma função. O `.s` e o `.b` gravam a mesma tabela.
pub(crate) struct FunctionSymbols<'a> {
    pub name: &'a str,
    pub name_position: Position,
    pub parameters: &'a [usize],
    pub result: Option<Type>,
    pub variables: &'a [Variable],
}

/// A tabela de símbolos. As colunas são alinhadas só para leitura; a última de cada linha
/// vai até o fim dela, porque pode ter espaços.
pub(crate) fn render_symbols(output: &mut String, functions: &[FunctionSymbols]) {
    let width = |title: &str, values: &mut dyn Iterator<Item = usize>| {
        values.fold(title.chars().count(), usize::max)
    };
    let position = |position: Position| format!("{}:{}", position.line, position.column);

    let name_width = width("#NOME", &mut functions.iter().map(|f| f.name.len()));
    output.push_str("[funções]\n");
    output.push_str(&format!(
        "{:<name_width$}  DECLARAÇÃO  ASSINATURA\n",
        "#NOME"
    ));
    for function in functions {
        let parameters: Vec<String> = function
            .parameters
            .iter()
            .map(|&index| function.variables[index].ty.to_string())
            .collect();
        let result = match function.result {
            Some(ty) => format!(" -> {ty}"),
            None => String::new(),
        };
        output.push_str(&format!(
            "{:<name_width$}  {:<10}  ({}){result}\n",
            function.name,
            position(function.name_position),
            parameters.join(", ")
        ));
    }

    for function in functions {
        let variables = function.variables;
        let number_width = width("#N", &mut (0..variables.len()).map(|n| n.to_string().len()));
        let name_width = width("NOME", &mut variables.iter().map(|v| v.name.len()));
        output.push_str(&format!("\n[variáveis {}]\n", function.name));
        output.push_str(&format!(
            "{:<number_width$}  {:<name_width$}  TIPO  DECLARAÇÃO  ESPÉCIE\n",
            "#N", "NOME"
        ));
        for (index, variable) in variables.iter().enumerate() {
            let kind = match (variable.parameter, variable.mutable) {
                (true, _) => "parâmetro",
                (false, false) => "let",
                (false, true) => "let mut",
            };
            output.push_str(&format!(
                "{index:<number_width$}  {:<name_width$}  {:<4}  {:<10}  {kind}\n",
                variable.name,
                variable.ty.to_string(),
                position(variable.position)
            ));
        }
    }
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
        self.line(function.position, 0, &format!("Função {}", function.name));
        for &index in &function.parameters {
            let text = format!("Parâmetro {}", self.variable(index));
            self.line(function.variables[index].position, 1, &text);
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
                let text = format!("Let {}", self.variable(*variable));
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
// pink-resume: Lê um arquivo .s de volta para a árvore verificada: a tabela de símbolos e a árvore que a consulta.
// pink-keywords: read, leitura, arquivo .s, árvore verificada, tabela de símbolos, seção, variável, tipo, chamada
/// Lê um arquivo `.s` (sem conferir o cabeçalho, que `artifact::detect` já leu). As
/// funções e as variáveis vêm da tabela de símbolos; a árvore precisa concordar com ela.
/// Confere a forma, as seções e os vínculos entre a árvore e a tabela; a coerência dos
/// tipos e do fluxo fica para o verificador (`verifier::verify`), que precisa rodar antes
/// da tradução para bytecode ou do interpretador.
pub fn read(content: &str) -> Result<CheckedProgram, FormatError> {
    let sections = sections(content, "árvore", "[árvore]")?;
    let tree = match sections.body.as_slice() {
        [(_, None, lines)] => lines.clone(),
        [(_, None, _), (line, ..), ..] => {
            return Err(format_error(
                *line,
                "a seção [árvore] aparece mais de uma vez",
            ));
        }
        [(line, Some(name), _), ..] => {
            return Err(format_error(
                *line,
                format!("seção '[árvore {name}]' desconhecida; esperava [árvore]"),
            ));
        }
        [] => unreachable!("sections exige a seção do corpo"),
    };
    let Symbols {
        functions: symbols,
        tables,
        by_name: functions_by_name,
    } = read_symbols(&sections)?;

    let mut reader = TreeReader::from_lines(tree)?;
    let mut functions = Vec::new();
    for (symbol, table) in symbols.iter().zip(tables) {
        let line = reader.child(0)?.ok_or_else(|| {
            format_error(
                sections.body_title,
                format!("falta a função '{}' na árvore", symbol.name),
            )
        })?;
        let (rows, variables): (Vec<usize>, Vec<Variable>) = table.into_iter().unzip();
        let function = FunctionReader {
            reader: &mut reader,
            functions: &functions_by_name,
            symbol,
            declared: vec![false; variables.len()],
            rows,
            variables,
        }
        .function(line)?;
        functions.push(function);
    }
    if let Some(line) = reader.child(0)? {
        return Err(format_error(
            line.line,
            format!(
                "a árvore tem uma função a mais que a tabela: '{}'",
                line.text
            ),
        ));
    }
    let main = *functions_by_name
        .get("main")
        .ok_or_else(|| format_error(1, "o programa não tem uma função 'main'"))?;
    Ok(CheckedProgram { functions, main })
}

/// A tabela de símbolos lida de um arquivo: as funções, as variáveis de cada uma (com a
/// linha de cada variável na tabela) e o índice de cada função pelo nome.
pub(crate) struct Symbols {
    pub functions: Vec<FunctionSymbol>,
    pub tables: Vec<Vec<(usize, Variable)>>,
    pub by_name: HashMap<String, usize>,
}

/// Lê as seções `[funções]` e `[variáveis <função>]`, conferindo que há uma seção de
/// variáveis por função, na ordem da tabela de funções.
pub(crate) fn read_symbols(sections: &Sections) -> Result<Symbols, FormatError> {
    let mut symbols = Vec::new();
    let mut functions_by_name = HashMap::new();
    for &(line, text) in &sections.functions {
        let symbol = function_symbol(line, text)?;
        if functions_by_name
            .insert(symbol.name.clone(), symbols.len())
            .is_some()
        {
            return Err(format_error(
                line,
                format!("função '{}' repetida", symbol.name),
            ));
        }
        symbols.push(symbol);
    }

    // Uma seção de variáveis por função, na ordem da tabela de funções.
    let mut tables = Vec::new();
    for (index, (line, name, rows)) in sections.variables.iter().enumerate() {
        match symbols.get(index) {
            Some(symbol) if symbol.name == *name => {}
            Some(symbol) => {
                return Err(format_error(
                    *line,
                    format!(
                        "esperava a seção [variáveis {}], na ordem da tabela de funções",
                        symbol.name
                    ),
                ));
            }
            None => {
                return Err(format_error(
                    *line,
                    format!("a função '{name}' não está na tabela de funções"),
                ));
            }
        }
        let table = rows
            .iter()
            .enumerate()
            .map(|(index, &(line, text))| Ok((line, variable_symbol(index, line, text)?)))
            .collect::<Result<Vec<_>, FormatError>>()?;
        tables.push(table);
    }
    if let Some(symbol) = symbols.get(tables.len()) {
        return Err(format_error(
            sections.body_title,
            format!("falta a seção [variáveis {}]", symbol.name),
        ));
    }
    Ok(Symbols {
        functions: symbols,
        tables,
        by_name: functions_by_name,
    })
}

pub(crate) type Lines<'a> = Vec<(usize, &'a str)>;

/// As linhas de cada seção, sem comentários e sem linhas em branco.
pub(crate) struct Sections<'a> {
    pub functions: Lines<'a>,
    /// Uma por seção `[variáveis <função>]`: a linha do título, o nome e as linhas.
    pub variables: Vec<(usize, &'a str, Lines<'a>)>,
    /// A linha do título da primeira seção do corpo.
    pub body_title: usize,
    /// As seções do corpo (`[árvore]` no `.s`, `[código <função>]` no `.b`): a linha do
    /// título, o nome depois da palavra do título, se houver, e as linhas.
    pub body: Vec<(usize, Option<&'a str>, Lines<'a>)>,
}

/// Separa as seções. A ordem é fixa: `[funções]`, as `[variáveis <função>]` e as seções
/// do corpo, cujo título começa com `body` (`order` descreve o corpo nas mensagens).
pub(crate) fn sections<'a>(
    content: &'a str,
    body: &str,
    order: &str,
) -> Result<Sections<'a>, FormatError> {
    #[derive(PartialEq)]
    enum Section {
        None,
        Functions,
        Variables,
        Body,
    }
    let mut sections = Sections {
        functions: Vec::new(),
        variables: Vec::new(),
        body_title: 0,
        body: Vec::new(),
    };
    let mut current = Section::None;
    for (line, text) in content_lines(content) {
        if let Some(title) = text
            .trim_end()
            .strip_prefix('[')
            .and_then(|t| t.strip_suffix(']'))
        {
            let started = matches!(current, Section::Functions | Section::Variables);
            let body_name = match title.strip_prefix(body) {
                Some("") => Some(None),
                Some(rest) => rest.strip_prefix(' ').map(Some),
                None => None,
            };
            match (title.strip_prefix("variáveis "), body_name) {
                _ if title == "funções" && current == Section::None => {
                    current = Section::Functions;
                }
                (_, Some(name)) if started || current == Section::Body => {
                    if current != Section::Body {
                        sections.body_title = line;
                    }
                    sections.body.push((line, name, Vec::new()));
                    current = Section::Body;
                }
                (Some(name), _) if started => {
                    sections.variables.push((line, name, Vec::new()));
                    current = Section::Variables;
                }
                _ => {
                    return Err(format_error(
                        line,
                        format!(
                            "seção '[{title}]' desconhecida ou fora de ordem; a ordem é [funções], [variáveis <função>] e {order}"
                        ),
                    ));
                }
            }
            continue;
        }
        match current {
            Section::None => return Err(format_error(line, "esperava a seção [funções]")),
            Section::Functions => sections.functions.push((line, text)),
            Section::Variables => {
                let last = sections.variables.last_mut().expect("seção aberta");
                last.2.push((line, text));
            }
            Section::Body => {
                let last = sections.body.last_mut().expect("seção aberta");
                last.2.push((line, text));
            }
        }
    }
    if current != Section::Body {
        let last = content.lines().count().max(1);
        return Err(format_error(last, format!("falta a seção {order}")));
    }
    Ok(sections)
}

/// Uma linha da tabela de funções.
pub(crate) struct FunctionSymbol {
    pub name: String,
    pub position: Position,
    pub parameters: Vec<Type>,
    pub result: Option<Type>,
    pub line: usize,
}

/// `nome  linha:coluna  (tipo, tipo) -> resultado`; sem resultado, termina no `)`.
fn function_symbol(line: usize, text: &str) -> Result<FunctionSymbol, FormatError> {
    let invalid = || format_error(line, "esperava 'nome  linha:coluna  (tipos) -> resultado'");
    let (name, rest) = split_field(text);
    let (position, signature) = split_field(rest);
    let (parameters, result) = signature
        .strip_prefix('(')
        .and_then(|signature| signature.split_once(')'))
        .ok_or_else(invalid)?;
    let parameters = match parameters.trim() {
        "" => Vec::new(),
        list => list
            .split(',')
            .map(|ty| parse_type(ty.trim(), line))
            .collect::<Result<_, _>>()?,
    };
    let result = match result.trim() {
        "" => None,
        arrow => {
            let ty = arrow.strip_prefix("->").ok_or_else(invalid)?;
            Some(parse_type(ty.trim(), line)?)
        }
    };
    Ok(FunctionSymbol {
        name: identifier(name, line)?.to_string(),
        position: parse_position(position, line)?,
        parameters,
        result,
        line,
    })
}

/// `n  nome  tipo  linha:coluna  espécie`, em que `n` precisa ser o número da linha na
/// seção, a partir de 0, e a espécie é `parâmetro`, `let` ou `let mut`.
fn variable_symbol(index: usize, line: usize, text: &str) -> Result<Variable, FormatError> {
    let (number, rest) = split_field(text);
    let (name, rest) = split_field(rest);
    let (ty, rest) = split_field(rest);
    let (position, kind) = split_field(rest);
    if number != index.to_string() {
        return Err(format_error(
            line,
            format!("esperava a variável #{index}, encontrei '{number}'"),
        ));
    }
    let (parameter, mutable) = match kind.trim_end() {
        "parâmetro" => (true, false),
        "let" => (false, false),
        "let mut" => (false, true),
        kind => {
            return Err(format_error(
                line,
                format!("espécie desconhecida '{kind}'; as espécies são parâmetro, let e let mut"),
            ));
        }
    };
    Ok(Variable {
        name: identifier(name, line)?.to_string(),
        ty: parse_type(ty, line)?,
        mutable,
        parameter,
        position: parse_position(position, line)?,
    })
}

pub(crate) fn parse_type(text: &str, line: usize) -> Result<Type, FormatError> {
    Type::from_name(text).ok_or_else(|| format_error(line, format!("tipo desconhecido '{text}'")))
}

/// `nome#n`: o nome e o número da variável.
pub(crate) fn variable_reference(text: &str, line: usize) -> Result<(&str, usize), FormatError> {
    let invalid = || format_error(line, format!("esperava nome#número, encontrei '{text}'"));
    let (name, index) = text.split_once('#').ok_or_else(invalid)?;
    let index = index.parse().map_err(|_| invalid())?;
    Ok((identifier(name, line)?, index))
}

struct FunctionReader<'r, 'a> {
    reader: &'r mut TreeReader<'a>,
    functions: &'r HashMap<String, usize>,
    symbol: &'r FunctionSymbol,
    /// As variáveis da tabela, pelo número, e a linha de cada uma na tabela.
    variables: Vec<Variable>,
    rows: Vec<usize>,
    /// Quais variáveis já têm a sua declaração (`Parâmetro` ou `Let`) na árvore.
    declared: Vec<bool>,
}

impl<'a> FunctionReader<'_, 'a> {
    fn function(mut self, line: TreeLine<'a>) -> Result<CheckedFunction, FormatError> {
        let name = expect_word(&line, "Função")?;
        if name != self.symbol.name {
            return Err(format_error(
                line.line,
                format!(
                    "esperava a função '{}', na ordem da tabela de funções; encontrei '{name}'",
                    self.symbol.name
                ),
            ));
        }
        let mut parameters = Vec::new();
        while self
            .reader
            .peek_child(1)
            .is_some_and(|child| split_word(child.text).0 == "Parâmetro")
        {
            let child = self.reader.child(1)?.expect("peek_child confirmou a linha");
            let text = expect_word(&child, "Parâmetro")?;
            parameters.push(self.declare(text, true, child.line)?);
        }
        let body = self.reader.expect_child(1, "o bloco da função", &line)?;
        let body = self.block(body, 1)?;
        self.reader.end(1)?;

        if let Some(index) = self.declared.iter().position(|declared| !declared) {
            return Err(format_error(
                self.rows[index],
                format!(
                    "a variável #{index} de '{name}' está na tabela, mas a árvore não a declara"
                ),
            ));
        }
        let types: Vec<Type> = parameters.iter().map(|&i| self.variables[i].ty).collect();
        if types != self.symbol.parameters {
            let list = |types: &[Type]| {
                let names: Vec<String> = types.iter().map(Type::to_string).collect();
                names.join(", ")
            };
            return Err(format_error(
                self.symbol.line,
                format!(
                    "a assinatura de '{name}' diz ({}), mas os parâmetros na árvore são ({})",
                    list(&self.symbol.parameters),
                    list(&types)
                ),
            ));
        }
        Ok(CheckedFunction {
            name: name.to_string(),
            parameters,
            result: self.symbol.result,
            variables: self.variables,
            body,
            position: line.position,
            name_position: self.symbol.position,
        })
    }

    /// A declaração de uma variável na árvore: precisa ter a espécie da tabela e aparecer
    /// uma vez só.
    fn declare(&mut self, text: &str, parameter: bool, line: usize) -> Result<usize, FormatError> {
        let index = self.reference(text, line)?;
        if self.variables[index].parameter != parameter {
            let message = if parameter {
                format!("a variável #{index} não é um parâmetro na tabela")
            } else {
                format!("a variável #{index} é um parâmetro na tabela; declare-a com Parâmetro")
            };
            return Err(format_error(line, message));
        }
        if self.declared[index] {
            return Err(format_error(
                line,
                format!("a variável #{index} é declarada mais de uma vez"),
            ));
        }
        self.declared[index] = true;
        Ok(index)
    }

    /// Um `nome#n` da árvore: o número precisa existir na tabela, com o mesmo nome.
    fn reference(&mut self, text: &str, line: usize) -> Result<usize, FormatError> {
        let (name, index) = variable_reference(text, line)?;
        let declared = self.variables.get(index).ok_or_else(|| {
            format_error(
                line,
                format!(
                    "a variável #{index} não está na tabela de '{}'",
                    self.symbol.name
                ),
            )
        })?;
        if declared.name != name {
            return Err(format_error(
                line,
                format!(
                    "a variável #{index} se chama '{}', não '{name}'",
                    declared.name
                ),
            ));
        }
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
                let variable = self.declare(rest, false, line.line)?;
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
