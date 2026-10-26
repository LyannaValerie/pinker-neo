//! Arquivos do pipeline: o cabeçalho comum e a leitura das linhas dos arquivos `.t`
//! (tokens), `.a` (AST) e `.s` (árvore verificada).
//!
//! Cada etapa grava o seu resultado num arquivo que a etapa seguinte sabe ler. Os formatos
//! estão em `docs/arquivos.md`.

use crate::lexer;
use crate::token::{Keyword, Position, Token, TokenKind};

/// Versão dos formatos. Muda quando um formato muda de forma incompatível.
pub const VERSION: u32 = 1;

// pink-nav: start
// pink-resume: Formatos do pipeline, o cabeçalho e a detecção do formato de um arquivo pela primeira linha.
// pink-keywords: formato, cabeçalho, versão, fonte, extensão, detectar, erro de formato
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Format {
    Tokens,
    Ast,
    Semantic,
}

/// Cada formato, o nome usado em `--emit` e no cabeçalho, e a extensão do arquivo.
const FORMATS: &[(Format, &str, &str)] = &[
    (Format::Tokens, "tokens", "t"),
    (Format::Ast, "ast", "a"),
    (Format::Semantic, "semantic", "s"),
];

impl Format {
    pub fn from_name(name: &str) -> Option<Format> {
        FORMATS
            .iter()
            .find(|(_, candidate, _)| *candidate == name)
            .map(|(format, _, _)| *format)
    }

    pub fn name(self) -> &'static str {
        FORMATS.iter().find(|(f, _, _)| *f == self).unwrap().1
    }

    pub fn extension(self) -> &'static str {
        FORMATS.iter().find(|(f, _, _)| *f == self).unwrap().2
    }
}

/// Um problema na forma de um arquivo do pipeline, apontado pela linha do próprio arquivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    pub line: usize,
    pub message: String,
}

pub(crate) fn format_error(line: usize, message: impl Into<String>) -> FormatError {
    FormatError {
        line,
        message: message.into(),
    }
}

const PREFIX: &str = "# pinker ";
const SOURCE: &str = " · fonte: ";

/// A primeira linha de um arquivo do pipeline: o formato, a versão e o código-fonte de
/// origem, cujo nome aparece nos diagnósticos das etapas seguintes.
pub fn header(format: Format, source: &str) -> String {
    format!("{PREFIX}{} {VERSION}{SOURCE}{source}\n", format.name())
}

/// O formato e o código-fonte de um arquivo do pipeline. `Ok(None)` quando o conteúdo não
/// começa com um cabeçalho: nesse caso, é código-fonte Pinker.
pub fn detect(content: &str) -> Result<Option<(Format, String)>, FormatError> {
    let first = content.lines().next().unwrap_or("");
    let Some(rest) = first.strip_prefix(PREFIX) else {
        return Ok(None);
    };
    let invalid = || {
        format_error(
            1,
            format!("cabeçalho inválido; esperava '{PREFIX}<formato> <versão>{SOURCE}<arquivo>'"),
        )
    };
    let (description, source) = rest.split_once(SOURCE).ok_or_else(invalid)?;
    let (name, version) = description.split_once(' ').ok_or_else(invalid)?;
    let format = Format::from_name(name).ok_or_else(|| {
        format_error(
            1,
            format!("formato desconhecido '{name}'; os formatos são tokens, ast e semantic"),
        )
    })?;
    if version != VERSION.to_string() {
        return Err(format_error(
            1,
            format!(
                "a versão '{version}' do formato {name} não é suportada; esta versão do pinker lê a versão {VERSION}"
            ),
        ));
    }
    if source.is_empty() {
        return Err(invalid());
    }
    Ok(Some((format, source.to_string())))
}
// pink-nav: end

// pink-nav: start
// pink-resume: Leitura das linhas: comentários, posições, campos, literais e identificadores validados com as regras do lexer.
// pink-keywords: linha, comentário, posição, campo, literal, identificador, validação
/// As linhas com conteúdo, numeradas a partir de 1: sem o cabeçalho, sem comentários
/// (linhas que começam com `#`) e sem linhas em branco.
pub(crate) fn content_lines(content: &str) -> impl Iterator<Item = (usize, &str)> {
    content
        .lines()
        .enumerate()
        .skip(1)
        .map(|(index, text)| (index + 1, text))
        .filter(|(_, text)| !text.trim().is_empty() && !text.starts_with('#'))
}

pub(crate) fn parse_position(text: &str, line: usize) -> Result<Position, FormatError> {
    let parsed = text.split_once(':').and_then(|(l, c)| {
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !digits(l) || !digits(c) {
            return None;
        }
        Some(Position {
            line: l.parse().ok()?,
            column: c.parse().ok()?,
        })
    });
    match parsed {
        Some(position) if position.line >= 1 && position.column >= 1 => Ok(position),
        _ => Err(format_error(
            line,
            format!("posição inválida '{text}'; esperava linha:coluna"),
        )),
    }
}

/// O primeiro campo (até o primeiro espaço em branco) e o resto, sem os espaços iniciais.
pub(crate) fn split_field(text: &str) -> (&str, &str) {
    match text.find(char::is_whitespace) {
        Some(index) => (&text[..index], text[index..].trim_start()),
        None => (text, ""),
    }
}

/// Um nome escrito num arquivo: precisa seguir a regra dos identificadores do lexer.
pub(crate) fn identifier(text: &str, line: usize) -> Result<&str, FormatError> {
    let valid = text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && Keyword::from_word(text).is_none();
    if valid {
        Ok(text)
    } else {
        Err(format_error(
            line,
            format!("'{text}' não é um identificador válido"),
        ))
    }
}

/// Um literal inteiro ou de texto, lido pelo próprio lexer: as regras de prefixos,
/// dígitos e escapes são as mesmas do código-fonte.
pub(crate) fn literal(lexeme: &str, line: usize) -> Result<TokenKind, FormatError> {
    let tokens = lexer::tokenize(lexeme)
        .map_err(|e| format_error(line, format!("literal inválido '{lexeme}': {}", e.message)))?;
    match tokens.as_slice() {
        [
            Token {
                kind: kind @ (TokenKind::Integer { .. } | TokenKind::Text(_)),
                ..
            },
            newline,
            end,
        ] if newline.kind == TokenKind::Newline && end.kind == TokenKind::Eof => Ok(kind.clone()),
        _ => Err(format_error(
            line,
            format!("'{lexeme}' não é um literal inteiro nem de texto"),
        )),
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Linhas de árvore (.a e .s): gravação com indentação por nível e leitura com profundidade.
// pink-keywords: árvore, linha, indentação, profundidade, nível, filho, leitor
/// Grava uma linha de árvore: a posição numa coluna de 8 caracteres e o nó indentado com
/// dois espaços por nível.
pub(crate) fn tree_line(output: &mut String, position: Position, depth: usize, text: &str) {
    let position = format!("{}:{}", position.line, position.column);
    let indent = "  ".repeat(depth);
    output.push_str(&format!("{position:<8} {indent}{text}\n"));
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TreeLine<'a> {
    /// A linha no arquivo, para os erros de formato.
    pub line: usize,
    pub position: Position,
    pub depth: usize,
    pub text: &'a str,
}

/// Percorre as linhas de uma árvore, de cima para baixo, por nível de indentação.
pub(crate) struct TreeReader<'a> {
    lines: Vec<TreeLine<'a>>,
    index: usize,
}

impl<'a> TreeReader<'a> {
    pub fn new(content: &'a str) -> Result<Self, FormatError> {
        let lines = content_lines(content)
            .map(|(line, text)| tree_line_of(line, text))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TreeReader { lines, index: 0 })
    }

    pub fn lines(&self) -> &[TreeLine<'a>] {
        &self.lines
    }

    /// A próxima linha, se ela for um filho no nível `depth`. Um nível acima de `depth` é
    /// erro: a árvore pulou um nível.
    pub fn child(&mut self, depth: usize) -> Result<Option<TreeLine<'a>>, FormatError> {
        let Some(line) = self.lines.get(self.index).copied() else {
            return Ok(None);
        };
        if line.depth > depth {
            return Err(format_error(line.line, "indentação inesperada"));
        }
        if line.depth < depth {
            return Ok(None);
        }
        self.index += 1;
        Ok(Some(line))
    }

    /// A próxima linha, sem consumi-la, se ela for um filho no nível `depth`.
    pub fn peek_child(&self, depth: usize) -> Option<&TreeLine<'a>> {
        self.lines
            .get(self.index)
            .filter(|line| line.depth == depth)
    }

    pub fn expect_child(
        &mut self,
        depth: usize,
        what: &str,
        parent: &TreeLine,
    ) -> Result<TreeLine<'a>, FormatError> {
        self.child(depth)?
            .ok_or_else(|| format_error(parent.line, format!("falta {what} sob esta linha")))
    }

    /// Garante que não sobrou nenhum filho no nível `depth` ou abaixo dele.
    pub fn end(&self, depth: usize) -> Result<(), FormatError> {
        match self.lines.get(self.index) {
            Some(line) if line.depth >= depth => Err(format_error(
                line.line,
                format!("nó inesperado '{}'", line.text),
            )),
            _ => Ok(()),
        }
    }
}

/// Separa a posição, a profundidade e o texto de uma linha de árvore. O texto começa na
/// coluna seguinte à da posição (com pelo menos 8 caracteres) mais dois espaços por nível.
fn tree_line_of(line: usize, text: &str) -> Result<TreeLine<'_>, FormatError> {
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    let position_text = &text[..end];
    let position = parse_position(position_text, line)?;
    let rest = &text[end..];
    let spaces = rest.len() - rest.trim_start_matches(' ').len();
    if rest[spaces..].starts_with(char::is_whitespace) {
        return Err(format_error(line, "use só espaços na indentação"));
    }
    let column = end + spaces;
    let base = end.max(8) + 1;
    if column < base || !(column - base).is_multiple_of(2) {
        return Err(format_error(
            line,
            "indentação inválida: o nó começa depois da coluna da posição, com dois espaços por nível",
        ));
    }
    Ok(TreeLine {
        line,
        position,
        depth: (column - base) / 2,
        text: rest[spaces..].trim_end(),
    })
}

/// A primeira palavra de um nó e o resto do texto.
pub(crate) fn split_word(text: &str) -> (&str, &str) {
    text.split_once(' ').unwrap_or((text, ""))
}
// pink-nav: end
