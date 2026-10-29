//! Parser: transforma os tokens em uma árvore de sintaxe (AST).
//!
//! É um parser descendente recursivo: cada regra de `docs/gramatica.md` vira uma
//! função. Ele verifica só a estrutura. Tipos, nomes e regras de contexto (como `break`
//! fora de laço ou `give` incompatível com a função) ficam para a análise semântica.

use crate::ast::{
    BinaryOperator, Block, Call, Else, ElseBranch, Expression, ExpressionKind, Function, Name,
    Parameter, Program, Statement, StatementKind, UnaryOperator,
};
use crate::diagnostic::{Diagnostic, error};
use crate::token::{Keyword, Position, Symbol, Token, TokenKind};

// pink-nav: start
// pink-resume: Ponto de entrada do parser e as regras do nível do arquivo e das funções.
// pink-keywords: parse, parser, programa, função, parâmetro, bloco
/// Constrói a AST a partir dos tokens do lexer. Para no primeiro erro.
pub fn parse(tokens: &[Token]) -> Result<Program, Diagnostic> {
    Parser { tokens, index: 0 }.program()
}

struct Parser<'a> {
    /// Sempre termina com um token `Eof`, garantido pelo lexer.
    tokens: &'a [Token],
    index: usize,
}

impl<'a> Parser<'a> {
    fn program(&mut self) -> Result<Program, Diagnostic> {
        let mut functions = Vec::new();
        while self.peek().kind != TokenKind::Eof {
            if self.check_keyword(Keyword::Fun) {
                functions.push(self.function()?);
            } else if self.peek().kind == TokenKind::Indent {
                return Err(error("indentação inesperada", self.peek().position));
            } else {
                return Err(self.unexpected("'fun' no nível do arquivo"));
            }
        }
        Ok(Program { functions })
    }

    fn function(&mut self) -> Result<Function, Diagnostic> {
        let position = self.advance().position;
        let name = self.expect_name("o nome da função")?;
        self.expect_symbol(Symbol::LeftParen, "'(' depois do nome da função")?;
        let mut parameters = Vec::new();
        while !self.check_symbol(Symbol::RightParen) {
            let name = self.expect_name("o nome de um parâmetro")?;
            self.expect_symbol(Symbol::Colon, "':' e o tipo do parâmetro")?;
            let type_name = self.expect_name("o tipo do parâmetro")?;
            parameters.push(Parameter { name, type_name });
            if !self.eat_symbol(Symbol::Comma) {
                break;
            }
        }
        self.expect_symbol(Symbol::RightParen, "',' ou ')' na lista de parâmetros")?;
        let result = if self.eat_symbol(Symbol::Arrow) {
            Some(self.expect_name("o tipo do resultado depois de '->'")?)
        } else {
            None
        };
        self.expect_newline("o fim da linha depois do cabeçalho da função")?;
        let body = self.block()?;
        Ok(Function {
            name,
            parameters,
            result,
            body,
            position,
        })
    }

    /// Um bloco indentado, com pelo menos uma instrução.
    fn block(&mut self) -> Result<Block, Diagnostic> {
        if self.peek().kind != TokenKind::Indent {
            return Err(self.unexpected("um bloco indentado"));
        }
        let position = self.advance().position;
        let mut statements = Vec::new();
        while self.peek().kind != TokenKind::Dedent {
            statements.push(self.statement()?);
        }
        self.advance();
        Ok(Block {
            statements,
            position,
        })
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Instruções: let, atribuição, if/else, while, loop, break, continue, give e chamada.
    // pink-keywords: instrução, let, atribuição, if, else, while, loop, break, continue, give, chamada
    fn statement(&mut self) -> Result<Statement, Diagnostic> {
        let token = self.peek();
        let position = token.position;
        let kind = match &token.kind {
            TokenKind::Keyword(Keyword::Let) => self.let_statement()?,
            TokenKind::Keyword(Keyword::If) => return self.if_statement(),
            TokenKind::Keyword(Keyword::While) => {
                self.advance();
                let condition = self.expression()?;
                self.expect_newline("o fim da linha depois da condição")?;
                StatementKind::While {
                    condition,
                    body: self.block()?,
                }
            }
            TokenKind::Keyword(Keyword::Loop) => {
                self.advance();
                self.expect_newline("o fim da linha depois de 'loop'")?;
                StatementKind::Loop {
                    body: self.block()?,
                }
            }
            TokenKind::Keyword(Keyword::Break) => {
                self.advance();
                self.expect_newline("o fim da linha depois de 'break'")?;
                StatementKind::Break
            }
            TokenKind::Keyword(Keyword::Continue) => {
                self.advance();
                self.expect_newline("o fim da linha depois de 'continue'")?;
                StatementKind::Continue
            }
            TokenKind::Keyword(Keyword::Give) => {
                self.advance();
                let value = if self.peek().kind == TokenKind::Newline {
                    None
                } else {
                    Some(self.expression()?)
                };
                self.expect_newline("o fim da linha depois do valor de 'give'")?;
                StatementKind::Give { value }
            }
            TokenKind::Keyword(Keyword::Else) => {
                return Err(error("'else' sem 'if' correspondente", position));
            }
            TokenKind::Identifier(_) => self.name_statement()?,
            TokenKind::Indent => return Err(error("indentação inesperada", position)),
            _ => return Err(self.unexpected("uma instrução")),
        };
        Ok(Statement { kind, position })
    }

    fn let_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        self.advance();
        let mutable = self.eat_keyword(Keyword::Mut);
        let name = self.expect_name("o nome da variável")?;
        let type_name = if self.eat_symbol(Symbol::Colon) {
            Some(self.expect_name("o tipo da variável")?)
        } else {
            None
        };
        self.expect_symbol(Symbol::Equal, "'=' e o valor da variável")?;
        let value = self.expression()?;
        self.expect_newline("o fim da linha depois do valor")?;
        Ok(StatementKind::Let {
            mutable,
            name,
            type_name,
            value,
        })
    }

    fn if_statement(&mut self) -> Result<Statement, Diagnostic> {
        let position = self.advance().position;
        let condition = self.expression()?;
        self.expect_newline("o fim da linha depois da condição")?;
        let then_block = self.block()?;
        let else_branch = if self.check_keyword(Keyword::Else) {
            let else_position = self.advance().position;
            let branch = if self.check_keyword(Keyword::If) {
                ElseBranch::If(Box::new(self.if_statement()?))
            } else {
                self.expect_newline("'if' ou o fim da linha depois de 'else'")?;
                ElseBranch::Block(self.block()?)
            };
            Some(Else {
                branch,
                position: else_position,
            })
        } else {
            None
        };
        Ok(Statement {
            kind: StatementKind::If {
                condition,
                then_block,
                else_branch,
            },
            position,
        })
    }

    /// Uma instrução que começa com um nome: atribuição (`x = ...`) ou chamada (`f(...)`).
    fn name_statement(&mut self) -> Result<StatementKind, Diagnostic> {
        let name = self.expect_name("um nome")?;
        if self.eat_symbol(Symbol::Equal) {
            let value = self.expression()?;
            self.expect_newline("o fim da linha depois do valor")?;
            return Ok(StatementKind::Assign {
                target: name,
                value,
            });
        }
        if !self.check_symbol(Symbol::LeftParen) {
            return Err(self.unexpected(&format!(
                "'=' ou '(' depois de '{}' (uma instrução que começa com um nome é uma atribuição ou uma chamada)",
                name.text
            )));
        }
        let call = self.call(name)?;
        self.expect_newline(
            "o fim da linha (o valor de uma expressão maior que uma chamada seria descartado)",
        )?;
        Ok(StatementKind::Call(call))
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Expressões por nível de precedência, de or até as primárias, com comparações não associativas.
    // pink-keywords: expressão, precedência, operador, comparação, unário, binário, chamada, literal
    fn expression(&mut self) -> Result<Expression, Diagnostic> {
        self.or()
    }

    fn or(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.and()?;
        while self.check_keyword(Keyword::Or) {
            let position = self.advance().position;
            let right = self.and()?;
            left = binary(BinaryOperator::Or, left, right, position);
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.not()?;
        while self.check_keyword(Keyword::And) {
            let position = self.advance().position;
            let right = self.not()?;
            left = binary(BinaryOperator::And, left, right, position);
        }
        Ok(left)
    }

    /// `not` fica abaixo das comparações: `not a == b` é `not (a == b)`.
    fn not(&mut self) -> Result<Expression, Diagnostic> {
        if self.check_keyword(Keyword::Not) {
            let position = self.advance().position;
            let operand = self.not()?;
            return Ok(unary(UnaryOperator::Not, operand, position));
        }
        self.comparison()
    }

    /// Comparações não se encadeiam: `a < b < c` é erro de sintaxe.
    fn comparison(&mut self) -> Result<Expression, Diagnostic> {
        let left = self.additive()?;
        let Some(operator) = self.comparison_operator() else {
            return Ok(left);
        };
        let position = self.advance().position;
        let right = self.additive()?;
        if self.comparison_operator().is_some() {
            return Err(error(
                "comparações não se encadeiam; use parênteses ou 'and'",
                self.peek().position,
            ));
        }
        Ok(binary(operator, left, right, position))
    }

    fn comparison_operator(&self) -> Option<BinaryOperator> {
        match self.peek().kind {
            TokenKind::Symbol(Symbol::EqualEqual) => Some(BinaryOperator::Equal),
            TokenKind::Symbol(Symbol::BangEqual) => Some(BinaryOperator::NotEqual),
            TokenKind::Symbol(Symbol::Less) => Some(BinaryOperator::Less),
            TokenKind::Symbol(Symbol::LessEqual) => Some(BinaryOperator::LessEqual),
            TokenKind::Symbol(Symbol::Greater) => Some(BinaryOperator::Greater),
            TokenKind::Symbol(Symbol::GreaterEqual) => Some(BinaryOperator::GreaterEqual),
            _ => None,
        }
    }

    fn additive(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.multiplicative()?;
        loop {
            let operator = match self.peek().kind {
                TokenKind::Symbol(Symbol::Plus) => BinaryOperator::Add,
                TokenKind::Symbol(Symbol::Minus) => BinaryOperator::Subtract,
                _ => return Ok(left),
            };
            let position = self.advance().position;
            let right = self.multiplicative()?;
            left = binary(operator, left, right, position);
        }
    }

    fn multiplicative(&mut self) -> Result<Expression, Diagnostic> {
        let mut left = self.negation()?;
        loop {
            let operator = match self.peek().kind {
                TokenKind::Symbol(Symbol::Star) => BinaryOperator::Multiply,
                TokenKind::Symbol(Symbol::Slash) => BinaryOperator::Divide,
                TokenKind::Symbol(Symbol::Percent) => BinaryOperator::Remainder,
                _ => return Ok(left),
            };
            let position = self.advance().position;
            let right = self.negation()?;
            left = binary(operator, left, right, position);
        }
    }

    /// O `-` prefixo: `-a * b` é `(-a) * b`.
    fn negation(&mut self) -> Result<Expression, Diagnostic> {
        if self.check_symbol(Symbol::Minus) {
            let position = self.advance().position;
            let operand = self.negation()?;
            return Ok(unary(UnaryOperator::Negate, operand, position));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expression, Diagnostic> {
        let token = self.peek();
        let position = token.position;
        let kind = match &token.kind {
            TokenKind::Integer { base, digits } => ExpressionKind::Integer {
                base: *base,
                digits: digits.clone(),
            },
            TokenKind::Text(value) => ExpressionKind::Text(value.clone()),
            TokenKind::Keyword(Keyword::True) => ExpressionKind::Bool(true),
            TokenKind::Keyword(Keyword::False) => ExpressionKind::Bool(false),
            TokenKind::Identifier(_) => {
                let name = self.expect_name("um nome")?;
                let kind = if self.check_symbol(Symbol::LeftParen) {
                    ExpressionKind::Call(self.call(name)?)
                } else {
                    ExpressionKind::Name(name.text)
                };
                return Ok(Expression { kind, position });
            }
            TokenKind::Symbol(Symbol::LeftParen) => {
                self.advance();
                let inner = self.expression()?;
                self.expect_symbol(Symbol::RightParen, "')' para fechar a expressão")?;
                return Ok(inner);
            }
            _ => return Err(self.missing_operand()),
        };
        self.advance();
        Ok(Expression { kind, position })
    }

    /// Os argumentos de uma chamada, a partir do `(` que segue o nome.
    fn call(&mut self, callee: Name) -> Result<Call, Diagnostic> {
        self.advance();
        let mut arguments = Vec::new();
        while !self.check_symbol(Symbol::RightParen) {
            arguments.push(self.expression()?);
            if !self.eat_symbol(Symbol::Comma) {
                break;
            }
        }
        self.expect_symbol(Symbol::RightParen, "',' ou ')' na lista de argumentos")?;
        Ok(Call { callee, arguments })
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Auxiliares de leitura dos tokens e de mensagens de erro ("esperava X, encontrei Y").
    // pink-keywords: peek, advance, expect, check, eat, erro, mensagem, unexpected
    fn peek(&self) -> &'a Token {
        let last = self.tokens.len() - 1;
        &self.tokens[self.index.min(last)]
    }

    fn advance(&mut self) -> &'a Token {
        let token = &self.tokens[self.index.min(self.tokens.len() - 1)];
        self.index += 1;
        token
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        self.peek().kind == TokenKind::Keyword(keyword)
    }

    fn check_symbol(&self, symbol: Symbol) -> bool {
        self.peek().kind == TokenKind::Symbol(symbol)
    }

    fn eat_keyword(&mut self, keyword: Keyword) -> bool {
        let found = self.check_keyword(keyword);
        if found {
            self.advance();
        }
        found
    }

    fn eat_symbol(&mut self, symbol: Symbol) -> bool {
        let found = self.check_symbol(symbol);
        if found {
            self.advance();
        }
        found
    }

    fn expect_symbol(&mut self, symbol: Symbol, expected: &str) -> Result<(), Diagnostic> {
        if self.eat_symbol(symbol) {
            Ok(())
        } else {
            Err(self.unexpected(expected))
        }
    }

    fn expect_name(&mut self, expected: &str) -> Result<Name, Diagnostic> {
        let token = self.peek();
        let TokenKind::Identifier(text) = &token.kind else {
            return Err(self.unexpected(expected));
        };
        let name = Name {
            text: text.clone(),
            position: token.position,
        };
        self.advance();
        Ok(name)
    }

    fn expect_newline(&mut self, expected: &str) -> Result<(), Diagnostic> {
        if self.peek().kind == TokenKind::Newline {
            self.advance();
            Ok(())
        } else {
            Err(self.unexpected(expected))
        }
    }

    fn unexpected(&self, expected: &str) -> Diagnostic {
        let token = self.peek();
        error(
            format!("esperava {expected}, encontrei {}", describe(token)),
            token.position,
        )
    }
    // pink-nav: end

    // pink-nav: start
    // pink-resume: Operando ausente: o erro de primary explica qual operador ficou sem operando, à esquerda ou à direita, o '+' prefixo e o 'not' sem parênteses.
    // pink-keywords: operando ausente, operador, expressão incompleta, + unário, not, precedência, diagnóstico
    /// O erro quando o token atual não começa uma expressão. Em `primary`, nada foi
    /// consumido depois do último operador: se o token anterior é um operador (binário,
    /// `-` ou `not`), é dele o operando que falta.
    fn missing_operand(&self) -> Diagnostic {
        let token = self.peek();
        let operator = self
            .index
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .filter(|previous| is_operator(previous));
        let message = if token.kind == TokenKind::Symbol(Symbol::Plus) {
            "a Pinker não tem '+' unário: '+' precisa de um operando à esquerda".to_string()
        } else if is_binary_only(token) {
            format!("falta o operando à esquerda de '{}'", token.lexeme)
        } else if let Some(operator) = operator {
            let operator = &operator.lexeme;
            if token.kind == TokenKind::Keyword(Keyword::Not) {
                format!(
                    "'not' tem precedência menor que '{operator}' e só pode ser operando dele entre parênteses"
                )
            } else if ends_operand(token) {
                format!(
                    "falta o operando à direita de '{operator}', encontrei {}",
                    describe(token)
                )
            } else {
                format!(
                    "esperava o operando à direita de '{operator}', encontrei {}",
                    describe(token)
                )
            }
        } else {
            return self.unexpected("uma expressão");
        };
        error(message, token.position)
    }
    // pink-nav: end
}

/// Como um token aparece nas mensagens de erro.
fn describe(token: &Token) -> String {
    match token.kind {
        TokenKind::Newline => "o fim da linha".to_string(),
        TokenKind::Indent => "uma indentação".to_string(),
        TokenKind::Dedent => "o fim do bloco".to_string(),
        TokenKind::Eof => "o fim do arquivo".to_string(),
        _ => format!("'{}'", token.lexeme),
    }
}

/// Os operadores que só existem entre dois operandos.
fn is_binary_only(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Symbol(
            Symbol::Star
                | Symbol::Slash
                | Symbol::Percent
                | Symbol::EqualEqual
                | Symbol::BangEqual
                | Symbol::Less
                | Symbol::LessEqual
                | Symbol::Greater
                | Symbol::GreaterEqual
        ) | TokenKind::Keyword(Keyword::And | Keyword::Or)
    )
}

/// Os operadores de expressões: os binários e os prefixos `-` e `not`.
fn is_operator(token: &Token) -> bool {
    is_binary_only(token)
        || matches!(
            token.kind,
            TokenKind::Symbol(Symbol::Plus | Symbol::Minus) | TokenKind::Keyword(Keyword::Not)
        )
}

/// Os tokens que encerram uma expressão: depois de um operador, mostram que o operando
/// não veio.
fn ends_operand(token: &Token) -> bool {
    matches!(
        token.kind,
        TokenKind::Newline
            | TokenKind::Dedent
            | TokenKind::Eof
            | TokenKind::Symbol(Symbol::RightParen | Symbol::Comma)
    )
}

fn binary(
    operator: BinaryOperator,
    left: Expression,
    right: Expression,
    position: Position,
) -> Expression {
    Expression {
        kind: ExpressionKind::Binary {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        },
        position,
    }
}

fn unary(operator: UnaryOperator, operand: Expression, position: Position) -> Expression {
    Expression {
        kind: ExpressionKind::Unary {
            operator,
            operand: Box::new(operand),
        },
        position,
    }
}
