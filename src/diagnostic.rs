//! Diagnósticos: os erros que as etapas do pipeline apontam no arquivo-fonte.

use crate::token::Position;

// pink-nav: start
// pink-resume: Erro de uma etapa do pipeline: a mensagem e a posição onde o problema foi encontrado.
// pink-keywords: erro, diagnóstico, posição, lexer, parser
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub position: Position,
}

pub(crate) fn error(message: impl Into<String>, position: Position) -> Diagnostic {
    Diagnostic {
        message: message.into(),
        position,
    }
}
// pink-nav: end
