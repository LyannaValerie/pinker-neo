//! Testes do lexer: entradas válidas e inválidas, com posições e diagnósticos.

use pinker::lexer::tokenize;
use pinker::token::{Position, Token, TokenKind};

// pink-nav: start
// pink-resume: Auxiliares que resumem tokens e erros em formas curtas para comparar nos testes.
// pink-keywords: auxiliar, helper, describe, kinds, error_at
/// Palavras reservadas e símbolos aparecem pela grafia; o resto, com um prefixo.
fn describe(token: &Token) -> String {
    match &token.kind {
        TokenKind::Keyword(_) | TokenKind::Symbol(_) => token.lexeme.clone(),
        TokenKind::Identifier(name) => format!("id:{name}"),
        TokenKind::Integer { base, digits } => format!("int{}:{digits}", base.radix()),
        TokenKind::Text(value) => format!("text:{value}"),
        TokenKind::Newline => "NL".to_string(),
        TokenKind::Indent => "INDENT".to_string(),
        TokenKind::Dedent => "DEDENT".to_string(),
        TokenKind::Eof => "EOF".to_string(),
    }
}

fn kinds(source: &str) -> Vec<String> {
    tokenize(source)
        .unwrap_or_else(|e| panic!("o lexer deveria aceitar {source:?}: {e:?}"))
        .iter()
        .map(describe)
        .collect()
}

/// A mensagem, a linha e a coluna do erro.
fn error_at(source: &str) -> (String, usize, usize) {
    let e = tokenize(source).expect_err("o lexer deveria rejeitar a entrada");
    (e.message, e.position.line, e.position.column)
}
// pink-nav: end

// pink-nav: start
// pink-resume: Indentação: blocos, linhas vazias e comentários, vários DEDENT, recuo inválido e tabulação.
// pink-keywords: teste, indentação, indent, dedent, tabulação, linha em branco
#[test]
fn funcao_com_bloco_indentado() {
    let source = "fun soma(a: int, b: int) -> int\n    give a + b\n";
    assert_eq!(
        kinds(source),
        [
            "fun", "id:soma", "(", "id:a", ":", "id:int", ",", "id:b", ":", "id:int", ")", "->",
            "id:int", "NL", "INDENT", "give", "id:a", "+", "id:b", "NL", "DEDENT", "EOF",
        ]
    );
}

#[test]
fn linhas_vazias_e_comentarios_nao_afetam_a_indentacao() {
    let source = "if x\n\n    # comentário\n        \n    say(x)\n";
    assert_eq!(
        kinds(source),
        [
            "if", "id:x", "NL", "INDENT", "id:say", "(", "id:x", ")", "NL", "DEDENT", "EOF"
        ]
    );
}

#[test]
fn varios_dedent_de_uma_vez() {
    let source = "if a\n    if b\n        c\nd\n";
    assert_eq!(
        kinds(source),
        [
            "if", "id:a", "NL", "INDENT", "if", "id:b", "NL", "INDENT", "id:c", "NL", "DEDENT",
            "DEDENT", "id:d", "NL", "EOF",
        ]
    );
}

#[test]
fn recuo_para_nivel_inexistente_e_erro() {
    let (message, line, column) = error_at("if a\n        b\n    c\n");
    assert_eq!((line, column), (3, 5));
    assert!(message.contains("níveis: 0, 8"), "{message}");
}

#[test]
fn tabulacao_na_indentacao_e_erro() {
    let (message, line, column) = error_at("if a\n\tb\n");
    assert_eq!((line, column), (2, 1));
    assert!(message.contains("tabulação"), "{message}");
}

#[test]
fn tabulacao_fora_da_indentacao_e_aceita() {
    assert_eq!(kinds("a\t= 1\n"), ["id:a", "=", "int10:1", "NL", "EOF"]);
    assert_eq!(kinds("a\n\t\nb\n"), ["id:a", "NL", "id:b", "NL", "EOF"]);
}
// pink-nav: end

// pink-nav: start
// pink-resume: Continuação de linha dentro de ( ) e [ ], e erros de delimitadores sem par.
// pink-keywords: teste, continuação, delimitador, parênteses, colchetes, não fechado
#[test]
fn linha_continua_dentro_de_parenteses() {
    let source = "let total = (\n    first\n  + second\n)\nsay(total)\n";
    assert_eq!(
        kinds(source),
        [
            "let",
            "id:total",
            "=",
            "(",
            "id:first",
            "+",
            "id:second",
            ")",
            "NL",
            "id:say",
            "(",
            "id:total",
            ")",
            "NL",
            "EOF",
        ]
    );
}

#[test]
fn linha_continua_dentro_de_colchetes() {
    let source = "let v = [\n    10,\n    20,\n]\n";
    assert_eq!(
        kinds(source),
        [
            "let", "id:v", "=", "[", "int10:10", ",", "int10:20", ",", "]", "NL", "EOF"
        ]
    );
}

#[test]
fn delimitador_aberto_no_fim_do_arquivo_e_erro() {
    let (message, line, column) = error_at("say(1\n");
    assert_eq!((line, column), (1, 4));
    assert!(message.contains("não foi fechado"), "{message}");
}

#[test]
fn delimitador_trocado_e_erro() {
    let (message, line, column) = error_at("say(1]\n");
    assert_eq!((line, column), (1, 6));
    assert!(message.contains("aberto em 1:4"), "{message}");
    let (message, line, column) = error_at("([)]");
    assert_eq!((line, column), (1, 3));
    assert!(message.contains("'[' aberto em 1:2"), "{message}");
}

#[test]
fn fechamento_sem_abertura_e_erro() {
    let (_, line, column) = error_at("x)\n");
    assert_eq!((line, column), (1, 2));
}
// pink-nav: end

// pink-nav: start
// pink-resume: Comentários e textos: escapes válidos, escape desconhecido e texto não fechado.
// pink-keywords: teste, comentário, texto, string, escape, aspas
#[test]
fn comentario_e_ignorado_mas_cerquilha_em_texto_nao() {
    assert_eq!(
        kinds("say(\"a # b\")  # fim\n"),
        ["id:say", "(", "text:a # b", ")", "NL", "EOF"]
    );
}

#[test]
fn delimitadores_dentro_de_texto_nao_contam() {
    assert_eq!(
        kinds("say(\"( ] [\")\n"),
        ["id:say", "(", "text:( ] [", ")", "NL", "EOF"]
    );
}

#[test]
fn escapes_sao_resolvidos() {
    let tokens = tokenize(r#""\n\t\\\"""#).unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Text("\n\t\\\"".to_string()));
    assert_eq!(tokens[0].lexeme, r#""\n\t\\\"""#);
}

#[test]
fn escape_desconhecido_e_erro() {
    let (message, line, column) = error_at(r#""a\qb""#);
    assert_eq!((line, column), (1, 3));
    assert!(message.contains("\\q"), "{message}");
}

#[test]
fn texto_nao_fechado_e_erro() {
    for source in ["\"abc\nx\n", "\"abc", "\"abc\\"] {
        let (message, line, column) = error_at(source);
        assert_eq!((line, column), (1, 1), "{source:?}");
        assert!(message.contains("não fechado"), "{message}");
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Inteiros: bases, grafia preservada sem conversão, separador de dígitos, e prefixos, dígitos e frações inválidos.
// pink-keywords: teste, número, inteiro, hexadecimal, binário, prefixo, i64, u64, separador, sublinhado
#[test]
fn inteiros_guardam_base_e_digitos() {
    assert_eq!(
        kinds("10 0xFF 0b1010 18446744073709551615"),
        [
            "int10:10",
            "int16:FF",
            "int2:1010",
            "int10:18446744073709551615",
            "NL",
            "EOF"
        ]
    );
    assert_eq!(tokenize("0xFF").unwrap()[0].lexeme, "0xFF");
}

#[test]
fn o_lexer_nao_aplica_a_negacao() {
    // O mínimo de i64 chega ao parser como '-' seguido da magnitude, sem ser rejeitado.
    assert_eq!(
        kinds("-9223372036854775808"),
        ["-", "int10:9223372036854775808", "NL", "EOF"]
    );
}

#[test]
fn prefixo_sem_digitos_e_erro() {
    let (message, line, column) = error_at("0x");
    assert_eq!((line, column), (1, 1));
    assert!(message.contains("sem dígitos"), "{message}");
}

#[test]
fn digito_invalido_aponta_o_caractere() {
    let (message, line, column) = error_at("0b102");
    assert_eq!((line, column), (1, 5));
    assert!(message.contains("binário"), "{message}");
    let (message, line, column) = error_at("0xG");
    assert_eq!((line, column), (1, 3));
    assert!(message.contains("hexadecimal"), "{message}");
    let (message, line, column) = error_at("123abc");
    assert_eq!((line, column), (1, 4));
    assert!(message.contains("decimal"), "{message}");
}

#[test]
fn prefixo_desconhecido_e_erro() {
    let (message, line, column) = error_at("0o17");
    assert_eq!((line, column), (1, 1));
    assert!(message.contains("'0o'"), "{message}");
}

#[test]
fn prefixos_aceitam_maiusculas() {
    assert_eq!(kinds("0XfF 0B10"), ["int16:fF", "int2:10", "NL", "EOF"]);
}

#[test]
fn zeros_a_esquerda_sao_decimais() {
    assert_eq!(kinds("010"), ["int10:010", "NL", "EOF"]);
}

#[test]
fn separador_entre_digitos_fica_so_na_grafia() {
    assert_eq!(
        kinds("1_000 0xFF_FF 0b1010_0000 1_2_3 0_1"),
        [
            "int10:1000",
            "int16:FFFF",
            "int2:10100000",
            "int10:123",
            "int10:01",
            "NL",
            "EOF"
        ]
    );
    assert_eq!(tokenize("0xFF_FF").unwrap()[0].lexeme, "0xFF_FF");
}

#[test]
fn separador_fora_do_lugar_aponta_o_sublinhado() {
    let separator = |source: &str| {
        let (message, line, column) = error_at(source);
        assert!(
            message.contains("o separador '_' só pode ficar entre dois dígitos"),
            "{source}: {message}"
        );
        (line, column)
    };
    assert_eq!(separator("1__000"), (1, 2));
    assert_eq!(separator("1000_"), (1, 5));
    assert_eq!(separator("0x_FF"), (1, 3));
    assert_eq!(separator("0b1_"), (1, 4));
    assert_eq!(separator("x = 7__"), (1, 6));
    // '_' sozinho no início não é um número: é o começo de um identificador.
    assert_eq!(kinds("_1"), ["id:_1", "NL", "EOF"]);
}

#[test]
fn o_erro_aponta_o_primeiro_problema_do_literal() {
    // Um separador entre um dígito e um caractere inválido não é o problema: o
    // caractere é.
    let (message, line, column) = error_at("1_x");
    assert_eq!((line, column), (1, 3));
    assert!(message.contains("'x' não é um dígito decimal"), "{message}");
    let (message, line, column) = error_at("0b1_2");
    assert_eq!((line, column), (1, 5));
    assert!(message.contains("binário"), "{message}");
    // Com os dois problemas, vale o que vem primeiro.
    let (message, line, column) = error_at("0__x");
    assert_eq!((line, column), (1, 2));
    assert!(message.contains("separador '_'"), "{message}");
    let (message, line, column) = error_at("0x_G");
    assert_eq!((line, column), (1, 3));
    assert!(message.contains("separador '_'"), "{message}");
    let (message, line, column) = error_at("1x__0");
    assert_eq!((line, column), (1, 2));
    assert!(message.contains("'x' não é um dígito decimal"), "{message}");
}

#[test]
fn parte_fracionaria_e_erro() {
    let (message, line, column) = error_at("x = 1.5");
    assert_eq!((line, column), (1, 5));
    assert!(message.contains("fracionária"), "{message}");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Palavras e símbolos: reservadas contra identificadores, operadores de dois caracteres e caracteres inválidos.
// pink-keywords: teste, palavra reservada, identificador, símbolo, operador, caractere inesperado
#[test]
fn tipos_e_intrinsecas_sao_identificadores() {
    assert_eq!(
        kinds("int i32 text say morph union propagate alloc free"),
        [
            "id:int",
            "id:i32",
            "id:text",
            "id:say",
            "id:morph",
            "id:union",
            "id:propagate",
            "id:alloc",
            "id:free",
            "NL",
            "EOF",
        ]
    );
}

#[test]
fn operadores_por_palavra_e_literais_sao_reservados() {
    assert_eq!(
        kinds("and or not true false self"),
        ["and", "or", "not", "true", "false", "self", "NL", "EOF"]
    );
}

#[test]
fn simbolos_de_um_e_de_dois_caracteres() {
    assert_eq!(
        kinds("a == b != c <= d >= e < f > g -> h = i % j / k * l . m"),
        [
            "id:a", "==", "id:b", "!=", "id:c", "<=", "id:d", ">=", "id:e", "<", "id:f", ">",
            "id:g", "->", "id:h", "=", "id:i", "%", "id:j", "/", "id:k", "*", "id:l", ".", "id:m",
            "NL", "EOF",
        ]
    );
}

#[test]
fn exclamacao_sozinha_sugere_not() {
    let (message, line, column) = error_at("not !x");
    assert_eq!((line, column), (1, 5));
    assert!(message.contains("not"), "{message}");
}

#[test]
fn caractere_inesperado_e_erro() {
    let (message, line, column) = error_at("x @ y");
    assert_eq!((line, column), (1, 3));
    assert!(message.contains("'@'"), "{message}");
    let (_, line, column) = error_at("é");
    assert_eq!((line, column), (1, 1));
}
// pink-nav: end

// pink-nav: start
// pink-resume: Posições dos tokens, fim de arquivo sem quebra final, arquivo vazio e quebras \r\n.
// pink-keywords: teste, posição, linha, coluna, fim de arquivo, crlf, arquivo vazio
#[test]
fn posicoes_contam_caracteres() {
    let tokens = tokenize("fun f()\n    say(\"olá\") # x\n").unwrap();
    let at = |lexeme: &str| {
        tokens
            .iter()
            .find(|t| t.lexeme == lexeme)
            .map(|t| t.position)
            .unwrap()
    };
    assert_eq!(at("say"), Position { line: 2, column: 5 });
    assert_eq!(at("\"olá\""), Position { line: 2, column: 9 });
    assert_eq!(at(")"), Position { line: 1, column: 7 });
    let newlines: Vec<Position> = tokens
        .iter()
        .filter(|t| t.kind == TokenKind::Newline)
        .map(|t| t.position)
        .collect();
    assert_eq!(
        newlines,
        [
            Position { line: 1, column: 8 },
            Position {
                line: 2,
                column: 19
            }
        ]
    );
    let indent = tokens.iter().find(|t| t.kind == TokenKind::Indent).unwrap();
    assert_eq!(indent.position, Position { line: 2, column: 5 });
}

#[test]
fn fim_de_arquivo_sem_quebra_fecha_linha_e_blocos() {
    assert_eq!(
        kinds("if a\n    b"),
        ["if", "id:a", "NL", "INDENT", "id:b", "NL", "DEDENT", "EOF"]
    );
}

#[test]
fn arquivo_vazio_ou_so_com_comentarios() {
    assert_eq!(kinds(""), ["EOF"]);
    assert_eq!(kinds("# só comentário\n\n"), ["EOF"]);
}

#[test]
fn quebras_crlf_sao_aceitas() {
    let tokens = tokenize("a\r\nb\r\n").unwrap();
    assert_eq!(
        tokens.iter().map(describe).collect::<Vec<_>>(),
        ["id:a", "NL", "id:b", "NL", "EOF"]
    );
    assert_eq!(tokens[2].position, Position { line: 2, column: 1 });
}

#[test]
fn retorno_de_carro_isolado_e_erro() {
    let (_, line, column) = error_at("a\rb");
    assert_eq!((line, column), (1, 2));
}
// pink-nav: end
