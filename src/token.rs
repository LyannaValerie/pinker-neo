//! Tokens: as unidades que o lexer reconhece no texto de um arquivo `.pk`.

use crate::artifact::{
    Format, FormatError, content_lines, format_error, header, identifier, literal, parse_position,
    split_field,
};

// pink-nav: start
// pink-resume: Posição de um token ou de um erro no arquivo-fonte.
// pink-keywords: posição, linha, coluna
/// Posição no arquivo-fonte. Linha e coluna começam em 1; a coluna conta caracteres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}
// pink-nav: end

// pink-nav: start
// pink-resume: Palavras reservadas da Pinker e busca de uma palavra pela grafia.
// pink-keywords: palavra reservada, keyword, operador por palavra, literal lógico
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Package,
    Bring,
    As,
    Public,
    Fun,
    Give,
    Let,
    Mut,
    Fixed,
    Shape,
    Enum,
    Alias,
    Pact,
    Impl,
    SelfValue,
    If,
    Else,
    While,
    For,
    In,
    Loop,
    Break,
    Continue,
    Match,
    When,
    Try,
    And,
    Or,
    Not,
    True,
    False,
}

/// Grafia de cada palavra reservada. Tipos (`int`, `text`...) e intrínsecas (`say`,
/// `morph`...) não estão aqui: são identificadores, resolvidos em etapas posteriores.
const KEYWORDS: &[(&str, Keyword)] = &[
    ("package", Keyword::Package),
    ("bring", Keyword::Bring),
    ("as", Keyword::As),
    ("public", Keyword::Public),
    ("fun", Keyword::Fun),
    ("give", Keyword::Give),
    ("let", Keyword::Let),
    ("mut", Keyword::Mut),
    ("fixed", Keyword::Fixed),
    ("shape", Keyword::Shape),
    ("enum", Keyword::Enum),
    ("alias", Keyword::Alias),
    ("pact", Keyword::Pact),
    ("impl", Keyword::Impl),
    ("self", Keyword::SelfValue),
    ("if", Keyword::If),
    ("else", Keyword::Else),
    ("while", Keyword::While),
    ("for", Keyword::For),
    ("in", Keyword::In),
    ("loop", Keyword::Loop),
    ("break", Keyword::Break),
    ("continue", Keyword::Continue),
    ("match", Keyword::Match),
    ("when", Keyword::When),
    ("try", Keyword::Try),
    ("and", Keyword::And),
    ("or", Keyword::Or),
    ("not", Keyword::Not),
    ("true", Keyword::True),
    ("false", Keyword::False),
];

impl Keyword {
    pub fn from_word(word: &str) -> Option<Keyword> {
        KEYWORDS
            .iter()
            .find(|(spelling, _)| *spelling == word)
            .map(|(_, keyword)| *keyword)
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Símbolos de pontuação e operadores, e busca de um símbolo pela grafia.
// pink-keywords: símbolo, operador, pontuação, delimitador, seta
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Symbol {
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Arrow,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Dot,
}

const SYMBOLS: &[(&str, Symbol)] = &[
    ("->", Symbol::Arrow),
    ("==", Symbol::EqualEqual),
    ("!=", Symbol::BangEqual),
    ("<=", Symbol::LessEqual),
    (">=", Symbol::GreaterEqual),
    ("+", Symbol::Plus),
    ("-", Symbol::Minus),
    ("*", Symbol::Star),
    ("/", Symbol::Slash),
    ("%", Symbol::Percent),
    ("=", Symbol::Equal),
    ("<", Symbol::Less),
    (">", Symbol::Greater),
    ("(", Symbol::LeftParen),
    (")", Symbol::RightParen),
    ("[", Symbol::LeftBracket),
    ("]", Symbol::RightBracket),
    (",", Symbol::Comma),
    (":", Symbol::Colon),
    (".", Symbol::Dot),
];

impl Symbol {
    pub fn from_spelling(spelling: &str) -> Option<Symbol> {
        SYMBOLS
            .iter()
            .find(|(candidate, _)| *candidate == spelling)
            .map(|(_, symbol)| *symbol)
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Base de um literal inteiro (decimal, hexadecimal, binária) e seus dígitos válidos.
// pink-keywords: base, decimal, hexadecimal, binário, dígito, radix
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    Decimal,
    Hexadecimal,
    Binary,
}

impl Base {
    pub fn radix(self) -> u32 {
        match self {
            Base::Decimal => 10,
            Base::Hexadecimal => 16,
            Base::Binary => 2,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Base::Decimal => "decimal",
            Base::Hexadecimal => "hexadecimal",
            Base::Binary => "binário",
        }
    }

    pub fn accepts(self, digit: char) -> bool {
        digit.is_digit(self.radix())
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: O token em si: o tipo, a posição e a grafia exata no texto-fonte.
// pink-keywords: token, tipo de token, lexema, indent, dedent, quebra, fim
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Keyword(Keyword),
    Identifier(String),
    /// Inteiro como foi escrito: a base e os dígitos, sem o prefixo. A conversão para um
    /// tipo e a verificação de faixa ficam para a análise semântica.
    Integer {
        base: Base,
        digits: String,
    },
    /// Texto com os escapes já resolvidos.
    Text(String),
    Symbol(Symbol),
    /// Fim de uma linha lógica.
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub position: Position,
    /// Grafia exata no texto-fonte; vazia para quebras, INDENT, DEDENT e fim.
    pub lexeme: String,
}
// pink-nav: end

// pink-nav: start
// pink-resume: Grava os tokens no formato .t (saída de --emit=tokens) e lê um arquivo .t de volta para o parser.
// pink-keywords: emit, render, read, formatação, leitura, arquivo .t, tokens
/// O arquivo `.t`: o cabeçalho e uma linha por token, com posição, tipo e grafia.
pub fn render(tokens: &[Token], source: &str) -> String {
    let mut output = header(Format::Tokens, source);
    output.push_str("#POSIÇÃO TIPO           LEXEMA\n");
    for token in tokens {
        let position = format!("{}:{}", token.position.line, token.position.column);
        let detail = match &token.kind {
            TokenKind::Integer { base, .. } => format!("  (base {})", base.radix()),
            _ => String::new(),
        };
        let kind = kind_name(&token.kind);
        let line = format!("{position:<8} {kind:<14} {}{detail}", token.lexeme);
        output.push_str(line.trim_end());
        output.push('\n');
    }
    output
}

fn kind_name(kind: &TokenKind) -> &'static str {
    match kind {
        TokenKind::Keyword(_) => "PALAVRA",
        TokenKind::Identifier(_) => "IDENTIFICADOR",
        TokenKind::Integer { .. } => "INTEIRO",
        TokenKind::Text(_) => "TEXTO",
        TokenKind::Symbol(_) => "SÍMBOLO",
        TokenKind::Newline => "QUEBRA",
        TokenKind::Indent => "INDENT",
        TokenKind::Dedent => "DEDENT",
        TokenKind::Eof => "FIM",
    }
}

/// Lê um arquivo `.t` (sem conferir o cabeçalho, que `artifact::detect` já leu). Cada
/// grafia é conferida com as regras do lexer; a anotação `(base N)` dos inteiros é só
/// informativa. O último token precisa ser `FIM`.
pub fn read(content: &str) -> Result<Vec<Token>, FormatError> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut last_line = 1;
    for (line, text) in content_lines(content) {
        last_line = line;
        if tokens
            .last()
            .is_some_and(|token| token.kind == TokenKind::Eof)
        {
            return Err(format_error(line, "token depois de FIM"));
        }
        let (position, rest) = split_field(text);
        let position = parse_position(position, line)?;
        let (kind_text, rest) = split_field(rest);
        let rest = rest.trim_end();
        let (kind, lexeme) = match kind_text {
            "PALAVRA" => {
                let keyword = Keyword::from_word(rest).ok_or_else(|| {
                    format_error(line, format!("'{rest}' não é uma palavra reservada"))
                })?;
                (TokenKind::Keyword(keyword), rest)
            }
            "IDENTIFICADOR" => (
                TokenKind::Identifier(identifier(rest, line)?.to_string()),
                rest,
            ),
            "INTEIRO" | "TEXTO" => {
                let lexeme = if kind_text == "INTEIRO" {
                    split_field(rest).0
                } else {
                    rest
                };
                let kind = literal(lexeme, line)?;
                if kind_name(&kind) != kind_text {
                    return Err(format_error(
                        line,
                        format!("'{lexeme}' não é um {kind_text}"),
                    ));
                }
                (kind, lexeme)
            }
            "SÍMBOLO" => {
                let symbol = Symbol::from_spelling(rest)
                    .ok_or_else(|| format_error(line, format!("'{rest}' não é um símbolo")))?;
                (TokenKind::Symbol(symbol), rest)
            }
            "QUEBRA" | "INDENT" | "DEDENT" | "FIM" => {
                if !rest.is_empty() {
                    return Err(format_error(
                        line,
                        format!("{kind_text} não tem lexema, mas encontrei '{rest}'"),
                    ));
                }
                let kind = match kind_text {
                    "QUEBRA" => TokenKind::Newline,
                    "INDENT" => TokenKind::Indent,
                    "DEDENT" => TokenKind::Dedent,
                    _ => TokenKind::Eof,
                };
                (kind, "")
            }
            other => {
                return Err(format_error(
                    line,
                    format!("tipo de token desconhecido '{other}'"),
                ));
            }
        };
        tokens.push(Token {
            kind,
            position,
            lexeme: lexeme.to_string(),
        });
    }
    if !tokens
        .last()
        .is_some_and(|token| token.kind == TokenKind::Eof)
    {
        return Err(format_error(
            last_line,
            "falta o token FIM no fim do arquivo",
        ));
    }
    Ok(tokens)
}

/// Um texto entre aspas, com os escapes da Pinker: o inverso da leitura de um literal.
pub fn quote_text(value: &str) -> String {
    let mut quoted = String::from('"');
    for c in value.chars() {
        match c {
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}
// pink-nav: end
