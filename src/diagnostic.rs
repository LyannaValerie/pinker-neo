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

// pink-nav: start
// pink-resume: Trecho do código-fonte sob um diagnóstico: a linha da posição e um ^ na coluna, com tabulações e controles apresentados de forma fixa.
// pink-keywords: trecho, marcador, linha, coluna, tabulação, controle, crlf, unicode, apresentação
/// Como uma tabulação aparece no trecho.
const TAB: &str = "    ";

/// A linha `position.line` de `source` e, embaixo, um `^` na coluna `position.column`:
///
/// ```text
///   2 |     @
///     |     ^
/// ```
///
/// `source` é o texto que as etapas receberam; a função não lê arquivos. Linhas e colunas
/// contam a partir de 1, e as colunas contam caracteres, como no lexer. As linhas são
/// separadas por `\n`, e um `\r` logo antes dele (CRLF) não aparece. A coluna pode ser a
/// logo depois do último caractere da linha. Se a linha ou a coluna não existirem, devolve
/// `None`.
///
/// Uma tabulação aparece como quatro espaços. Os caracteres de [`escaped`] aparecem como
/// `\u{...}`. O `^` fica depois da largura apresentada dos caracteres anteriores, contando
/// um por caractere: caracteres largos ou combinantes podem desalinhá-lo no terminal.
pub fn excerpt(source: &str, position: Position) -> Option<String> {
    let Position { line, column } = position;
    let mut lines = source.split('\n');
    let text = lines.nth(line.checked_sub(1)?)?;
    let text = match lines.next() {
        Some(_) => text.strip_suffix('\r').unwrap_or(text),
        None => text,
    };
    let shown: Vec<String> = text.chars().map(show).collect();
    if column == 0 || column > shown.len() + 1 {
        return None;
    }
    let before: usize = shown[..column - 1]
        .iter()
        .map(|piece| piece.chars().count())
        .sum();
    let number = line.to_string();
    Some(format!(
        "  {number} | {}\n  {} | {}^",
        shown.concat(),
        " ".repeat(number.len()),
        " ".repeat(before)
    ))
}

fn show(c: char) -> String {
    match c {
        '\t' => TAB.to_string(),
        c if escaped(c) => c.escape_unicode().to_string(),
        c => c.to_string(),
    }
}

/// Os caracteres que o trecho mostra escapados, para não chegarem ao terminal: os de
/// controle (menos a tabulação), os que mudam a direção do texto, os separadores de linha
/// e de parágrafo do Unicode e o BOM.
fn escaped(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2066}'..='\u{2069}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{FEFF}'
        )
}
// pink-nav: end

#[cfg(test)]
mod tests {
    use super::*;

    fn at(line: usize, column: usize) -> Position {
        Position { line, column }
    }

    // pink-nav: start
    // pink-resume: Testes do trecho: coluna em caracteres, Unicode, tabulação, controles, LF, CRLF, fim sem quebra, fim de entrada e posições inválidas.
    // pink-keywords: teste, trecho, marcador, coluna, unicode, tabulação, controle, crlf, posição inválida
    #[test]
    fn marca_a_coluna_na_linha_da_posicao() {
        let source = "fun main()\n    @\n";
        assert_eq!(
            excerpt(source, at(2, 5)).unwrap(),
            "  2 |     @\n    |     ^"
        );
    }

    #[test]
    fn coluna_conta_caracteres_e_nao_bytes() {
        // "ção" tem 3 caracteres e 5 bytes; 🌸 tem 1 caractere e 4 bytes.
        let source = "ação🌸 @\n";
        assert_eq!(
            excerpt(source, at(1, 7)).unwrap(),
            "  1 | ação🌸 @\n    |       ^"
        );
    }

    #[test]
    fn tabulacao_vira_quatro_espacos_e_o_marcador_acompanha() {
        let source = "x\ta\t@\n";
        assert_eq!(
            excerpt(source, at(1, 5)).unwrap(),
            format!("  1 | x    a    @\n    | {}^", " ".repeat(10))
        );
        // Na própria tabulação, o ^ fica no primeiro dos quatro espaços.
        assert_eq!(
            excerpt(source, at(1, 2)).unwrap(),
            "  1 | x    a    @\n    |  ^"
        );
    }

    #[test]
    fn controles_aparecem_escapados_e_o_marcador_acompanha() {
        let source = "a\u{1b}[31m\u{0}\u{202E}b\u{7f}@\n";
        // Antes do @ (coluna 11): a, \u{1b}, [31m, \u{0}, \u{202e}, b e \u{7f}.
        let shown = excerpt(source, at(1, 11)).unwrap();
        assert_eq!(
            shown,
            format!(
                "  1 | a\\u{{1b}}[31m\\u{{0}}\\u{{202e}}b\\u{{7f}}@\n    | {}^",
                " ".repeat(1 + 6 + 4 + 5 + 8 + 1 + 6)
            )
        );
        assert!(!shown.chars().any(|c| c != '\n' && escaped(c)), "{shown:?}");
        // Um \r isolado, que não faz parte de um CRLF, também é um controle.
        assert_eq!(
            excerpt("ab\rcd\n", at(1, 3)).unwrap(),
            "  1 | ab\\u{d}cd\n    |   ^"
        );
        assert_eq!(
            excerpt("ab\r", at(1, 3)).unwrap(),
            "  1 | ab\\u{d}\n    |   ^"
        );
    }

    #[test]
    fn crlf_nao_mostra_o_retorno() {
        let source = "fun main()\r\n    @\r\n";
        assert_eq!(
            excerpt(source, at(2, 5)).unwrap(),
            "  2 |     @\n    |     ^"
        );
        // A posição do \r de um CRLF é a logo depois do último caractere.
        assert_eq!(
            excerpt(source, at(1, 11)).unwrap(),
            "  1 | fun main()\n    |           ^"
        );
        // A do \n, depois dele, não existe na linha apresentada.
        assert_eq!(excerpt(source, at(1, 12)), None);
    }

    #[test]
    fn fim_sem_quebra_e_fim_de_entrada() {
        // Sem quebra final, a última linha existe e aceita a coluna depois do último
        // caractere.
        assert_eq!(
            excerpt("fun main()", at(1, 11)).unwrap(),
            "  1 | fun main()\n    |           ^"
        );
        // Com quebra final, o fim da entrada é a coluna 1 da linha seguinte, vazia.
        assert_eq!(
            excerpt("fun main()\n", at(2, 1)).unwrap(),
            "  2 | \n    | ^"
        );
        assert_eq!(excerpt("", at(1, 1)).unwrap(), "  1 | \n    | ^");
    }

    #[test]
    fn numero_da_linha_alarga_a_margem() {
        let source = "\n".repeat(11) + "@";
        assert_eq!(excerpt(&source, at(12, 1)).unwrap(), "  12 | @\n     | ^");
    }

    #[test]
    fn posicao_invalida_nao_tem_trecho() {
        let source = "fun main()\n    @\n";
        for position in [
            at(0, 1),
            at(1, 0),
            at(1, 12),
            at(4, 1),
            at(usize::MAX, 1),
            at(1, usize::MAX),
        ] {
            assert_eq!(excerpt(source, position), None, "{position:?}");
        }
    }
    // pink-nav: end
}
