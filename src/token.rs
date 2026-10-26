//! Tokens: as unidades que o lexer reconhece no texto de um arquivo `.pk`.

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
// pink-resume: Formata os tokens em texto legível para a saída de --emit=tokens.
// pink-keywords: emit, render, formatação, saída, tokens
/// Uma linha por token: posição, tipo e grafia.
pub fn render(tokens: &[Token]) -> String {
    let mut output = String::from("POSIÇÃO  TIPO           LEXEMA\n");
    for token in tokens {
        let position = format!("{}:{}", token.position.line, token.position.column);
        let (kind, detail) = match &token.kind {
            TokenKind::Keyword(_) => ("PALAVRA", String::new()),
            TokenKind::Identifier(_) => ("IDENTIFICADOR", String::new()),
            TokenKind::Integer { base, .. } => ("INTEIRO", format!("  (base {})", base.radix())),
            TokenKind::Text(_) => ("TEXTO", String::new()),
            TokenKind::Symbol(_) => ("SÍMBOLO", String::new()),
            TokenKind::Newline => ("QUEBRA", String::new()),
            TokenKind::Indent => ("INDENT", String::new()),
            TokenKind::Dedent => ("DEDENT", String::new()),
            TokenKind::Eof => ("FIM", String::new()),
        };
        let line = format!("{position:<8} {kind:<14} {}{detail}", token.lexeme);
        output.push_str(line.trim_end());
        output.push('\n');
    }
    output
}
// pink-nav: end
