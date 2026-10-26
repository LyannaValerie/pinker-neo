//! Lexer: transforma o texto de um arquivo `.pk` em uma sequência de tokens.
//!
//! O lexer só reconhece a forma do texto. Ele não decide se uma construção pode abrir
//! um bloco (parser), não interpreta `-` como negação (parser) e não converte inteiros
//! para um tipo (análise semântica): os literais guardam a base e os dígitos.

use crate::token::{Base, Keyword, Position, Symbol, Token, TokenKind};

// pink-nav: start
// pink-resume: Erro léxico: a mensagem e a posição onde o problema foi encontrado.
// pink-keywords: erro, diagnóstico, posição
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub position: Position,
}

fn error(message: impl Into<String>, position: Position) -> LexError {
    LexError {
        message: message.into(),
        position,
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Estado do lexer e laço principal, que despacha cada caractere para a regra certa.
// pink-keywords: tokenize, lexer, laço principal, fim de arquivo, estado
/// Transforma o texto-fonte em tokens. Para no primeiro erro.
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(source).run()
}

struct Lexer {
    chars: Vec<char>,
    index: usize,
    line: usize,
    column: usize,
    /// Larguras dos níveis de indentação abertos; o primeiro é sempre 0.
    indents: Vec<usize>,
    /// Delimitadores abertos e onde foram abertos. Dentro deles, a linha continua.
    delimiters: Vec<(char, Position)>,
    /// Verdadeiro quando a próxima linha ainda não começou a produzir tokens.
    at_line_start: bool,
    tokens: Vec<Token>,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Lexer {
            chars: source.chars().collect(),
            index: 0,
            line: 1,
            column: 1,
            indents: vec![0],
            delimiters: Vec::new(),
            at_line_start: true,
            tokens: Vec::new(),
        }
    }

    fn run(mut self) -> Result<Vec<Token>, LexError> {
        loop {
            if self.at_line_start && self.delimiters.is_empty() {
                self.indentation()?;
            }
            let Some(c) = self.peek() else { break };
            match c {
                '\n' | '\r' => self.newline()?,
                ' ' | '\t' => {
                    self.advance();
                }
                '#' => self.comment(),
                '"' => self.text()?,
                '0'..='9' => self.number()?,
                c if c.is_ascii_alphabetic() || c == '_' => self.word(),
                _ => self.symbol()?,
            }
        }
        self.finish()
    }

    /// Fecha o arquivo: a última linha lógica e os blocos ainda abertos.
    fn finish(mut self) -> Result<Vec<Token>, LexError> {
        if let Some((open, position)) = self.delimiters.last() {
            return Err(error(
                format!("'{open}' aberto aqui não foi fechado"),
                *position,
            ));
        }
        let end = self.position();
        if !self.at_line_start {
            self.push(TokenKind::Newline, end, String::new());
        }
        while self.indents.len() > 1 {
            self.indents.pop();
            self.push(TokenKind::Dedent, end, String::new());
        }
        self.push(TokenKind::Eof, end, String::new());
        Ok(self.tokens)
    }

    // pink-nav: start
    // pink-resume: Mede a indentação no início de cada linha e produz INDENT, DEDENT e quebras lógicas.
    // pink-keywords: indentação, indent, dedent, quebra de linha, tabulação, linha em branco, crlf
    /// Mede a indentação de uma linha. Linhas vazias ou só com comentário não abrem nem
    /// fecham blocos: o laço principal as consome sem produzir tokens.
    fn indentation(&mut self) -> Result<(), LexError> {
        let mut width = 0;
        let mut tab = None;
        while let Some(c @ (' ' | '\t')) = self.peek() {
            if c == '\t' && tab.is_none() {
                tab = Some(self.position());
            }
            width += 1;
            self.advance();
        }
        if matches!(self.peek(), None | Some('\n' | '\r' | '#')) {
            return Ok(());
        }
        if let Some(position) = tab {
            return Err(error("tabulação na indentação; use espaços", position));
        }

        self.at_line_start = false;
        let position = self.position();
        if width > self.current_indent() {
            self.indents.push(width);
            self.push(TokenKind::Indent, position, String::new());
            return Ok(());
        }
        if !self.indents.contains(&width) {
            let levels: Vec<String> = self.indents.iter().map(|level| level.to_string()).collect();
            return Err(error(
                format!(
                    "este recuo ({width} espaços) não volta a nenhum nível de indentação aberto (níveis: {})",
                    levels.join(", ")
                ),
                position,
            ));
        }
        while width < self.current_indent() {
            self.indents.pop();
            self.push(TokenKind::Dedent, position, String::new());
        }
        Ok(())
    }

    fn current_indent(&self) -> usize {
        *self.indents.last().expect("o nível 0 nunca é removido")
    }

    /// Consome uma quebra de linha (`\n` ou `\r\n`). Fora de delimitadores, encerra a
    /// linha lógica; dentro deles, a linha continua.
    fn newline(&mut self) -> Result<(), LexError> {
        let position = self.position();
        if self.advance() == Some('\r') {
            if self.peek() != Some('\n') {
                return Err(error(
                    "'\\r' isolado; quebre a linha com '\\n' ou '\\r\\n'",
                    position,
                ));
            }
            self.advance();
        }
        if self.delimiters.is_empty() {
            if !self.at_line_start {
                self.push(TokenKind::Newline, position, String::new());
            }
            self.at_line_start = true;
        }
        Ok(())
    }

    /// Ignora um comentário até o fim da linha, sem consumir a quebra.
    fn comment(&mut self) {
        while !matches!(self.peek(), None | Some('\n' | '\r')) {
            self.advance();
        }
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Lê um texto entre aspas duplas e resolve os escapes \n, \t, \\ e \".
    // pink-keywords: texto, string, aspas, escape, texto não fechado
    fn text(&mut self) -> Result<(), LexError> {
        let start = self.position();
        let begin = self.index;
        let unterminated = || error("texto não fechado: falta a aspa final '\"'", start);
        self.advance();
        let mut value = String::new();
        loop {
            match self.peek() {
                None | Some('\n' | '\r') => return Err(unterminated()),
                Some('"') => {
                    self.advance();
                    break;
                }
                Some('\\') => {
                    let escape_position = self.position();
                    self.advance();
                    let escaped = match self.peek() {
                        Some('n') => '\n',
                        Some('t') => '\t',
                        Some('\\') => '\\',
                        Some('"') => '"',
                        None | Some('\n' | '\r') => return Err(unterminated()),
                        Some(other) => {
                            return Err(error(
                                format!(
                                    "escape desconhecido '\\{other}'; os escapes válidos são \\n, \\t, \\\\ e \\\""
                                ),
                                escape_position,
                            ));
                        }
                    };
                    self.advance();
                    value.push(escaped);
                }
                Some(c) => {
                    self.advance();
                    value.push(c);
                }
            }
        }
        let lexeme = self.slice(begin);
        self.push(TokenKind::Text(value), start, lexeme);
        Ok(())
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Lê um inteiro decimal, hexadecimal (0x) ou binário (0b), guardando base e dígitos sem converter.
    // Zeros à esquerda são decimais: `010` é dez, nunca octal.
    // pink-keywords: número, inteiro, literal, hexadecimal, binário, prefixo, dígito inválido
    fn number(&mut self) -> Result<(), LexError> {
        let start = self.position();
        let begin = self.index;
        let base = match (self.peek(), self.peek_next()) {
            (Some('0'), Some('x' | 'X')) => Base::Hexadecimal,
            (Some('0'), Some('b' | 'B')) => Base::Binary,
            _ => Base::Decimal,
        };
        if base != Base::Decimal {
            self.advance();
            self.advance();
        }

        // Consome a sequência alfanumérica inteira, para apontar o caractere inválido
        // em vez de dividir `123abc` em dois tokens.
        let digits_begin = self.index;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
            self.advance();
        }
        let digits = self.slice(digits_begin);
        let lexeme = self.slice(begin);

        if digits.is_empty() {
            return Err(error(format!("prefixo '{lexeme}' sem dígitos"), start));
        }
        if base == Base::Decimal
            && digits.starts_with('0')
            && digits[1..].starts_with(|c: char| c.is_ascii_alphabetic())
        {
            return Err(error(
                format!(
                    "prefixo desconhecido '{}'; use 0x (hexadecimal) ou 0b (binário)",
                    &digits[..2]
                ),
                start,
            ));
        }
        if let Some((offset, digit)) = digits.chars().enumerate().find(|(_, c)| !base.accepts(*c)) {
            let position = Position {
                line: start.line,
                column: start.column + (digits_begin - begin) + offset,
            };
            return Err(error(
                format!("'{digit}' não é um dígito {}", base.name()),
                position,
            ));
        }
        if self.peek() == Some('.') && matches!(self.peek_next(), Some(c) if c.is_ascii_digit()) {
            return Err(error(
                "números com parte fracionária ainda não existem na Pinker",
                start,
            ));
        }

        self.push(TokenKind::Integer { base, digits }, start, lexeme);
        Ok(())
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Lê palavras (reservadas ou identificadores) e símbolos, e confere o pareamento de ( ) e [ ].
    // pink-keywords: identificador, palavra reservada, símbolo, operador, delimitador, parênteses, colchetes
    fn word(&mut self) {
        let start = self.position();
        let begin = self.index;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == '_') {
            self.advance();
        }
        let lexeme = self.slice(begin);
        let kind = match Keyword::from_word(&lexeme) {
            Some(keyword) => TokenKind::Keyword(keyword),
            None => TokenKind::Identifier(lexeme.clone()),
        };
        self.push(kind, start, lexeme);
    }

    fn symbol(&mut self) -> Result<(), LexError> {
        let start = self.position();
        let first = self
            .peek()
            .expect("o laço principal só chama symbol com caractere");
        if let Some(second) = self.peek_next() {
            let pair = format!("{first}{second}");
            if let Some(symbol) = Symbol::from_spelling(&pair) {
                self.advance();
                self.advance();
                self.push(TokenKind::Symbol(symbol), start, pair);
                return Ok(());
            }
        }

        let spelling = first.to_string();
        let Some(symbol) = Symbol::from_spelling(&spelling) else {
            let message = if first == '!' {
                "'!' sozinho não é um operador; para negação lógica use 'not'".to_string()
            } else {
                format!("caractere inesperado '{first}'")
            };
            return Err(error(message, start));
        };
        self.advance();
        match symbol {
            Symbol::LeftParen | Symbol::LeftBracket => self.delimiters.push((first, start)),
            Symbol::RightParen | Symbol::RightBracket => self.close(first, start)?,
            _ => {}
        }
        self.push(TokenKind::Symbol(symbol), start, spelling);
        Ok(())
    }

    fn close(&mut self, closing: char, position: Position) -> Result<(), LexError> {
        let expected = if closing == ')' { '(' } else { '[' };
        match self.delimiters.pop() {
            Some((open, _)) if open == expected => Ok(()),
            Some((open, open_position)) => Err(error(
                format!(
                    "'{closing}' não fecha o '{open}' aberto em {}:{}",
                    open_position.line, open_position.column
                ),
                position,
            )),
            None => Err(error(
                format!("'{closing}' sem '{expected}' correspondente"),
                position,
            )),
        }
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Leitura caractere a caractere, com controle de linha e coluna.
    // pink-keywords: peek, advance, posição, linha, coluna, lexema
    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.index += 1;
        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(c)
    }

    fn position(&self) -> Position {
        Position {
            line: self.line,
            column: self.column,
        }
    }

    /// O texto entre `begin` e a posição atual.
    fn slice(&self, begin: usize) -> String {
        self.chars[begin..self.index].iter().collect()
    }

    fn push(&mut self, kind: TokenKind, position: Position, lexeme: String) {
        self.tokens.push(Token {
            kind,
            position,
            lexeme,
        });
    }
    // pink-nav: end
}
// pink-nav: end
