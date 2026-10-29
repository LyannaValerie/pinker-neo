//! Testes do parser: estrutura da AST, precedência e rejeições sintáticas.

use pinker::ast::{self, ElseBranch, Expression, ExpressionKind, Program, StatementKind};
use pinker::lexer::tokenize;
use pinker::parser::parse;
use pinker::token::{Position, TokenKind};

// pink-nav: start
// pink-resume: Auxiliares: parse de um programa, resumo de expressões em S-expressões e posição do erro.
// pink-keywords: auxiliar, helper, sexpr, program, error_at
fn program(source: &str) -> Program {
    let tokens = tokenize(source).unwrap_or_else(|e| panic!("erro léxico em {source:?}: {e:?}"));
    parse(&tokens).unwrap_or_else(|e| panic!("o parser deveria aceitar {source:?}: {e:?}"))
}

/// A mensagem, a linha e a coluna do erro de sintaxe.
fn error_at(source: &str) -> (String, usize, usize) {
    let tokens = tokenize(source).unwrap_or_else(|e| panic!("erro léxico em {source:?}: {e:?}"));
    let e = parse(&tokens).expect_err("o parser deveria rejeitar a entrada");
    (e.message, e.position.line, e.position.column)
}

/// Uma expressão como S-expressão: `a + b * c` vira `(+ a (* b c))`.
fn sexpr(expression: &Expression) -> String {
    match &expression.kind {
        ExpressionKind::Integer { digits, .. } => digits.clone(),
        ExpressionKind::Text(value) => format!("{value:?}"),
        ExpressionKind::Bool(value) => value.to_string(),
        ExpressionKind::Name(name) => name.clone(),
        ExpressionKind::Call(call) => {
            let mut parts = vec!["call".to_string(), call.callee.text.clone()];
            parts.extend(call.arguments.iter().map(sexpr));
            format!("({})", parts.join(" "))
        }
        ExpressionKind::Unary { operator, operand } => {
            format!("({} {})", operator.as_str(), sexpr(operand))
        }
        ExpressionKind::Binary {
            operator,
            left,
            right,
        } => format!("({} {} {})", operator.as_str(), sexpr(left), sexpr(right)),
    }
}

/// A expressão de `give <expressão>` dentro de uma função.
fn expression(source: &str) -> String {
    let program = program(&format!("fun f()\n    give {source}\n"));
    match &program.functions[0].body.statements[0].kind {
        StatementKind::Give { value: Some(value) } => sexpr(value),
        other => panic!("esperava give com valor: {other:?}"),
    }
}

fn expression_error(source: &str) -> (String, usize) {
    let (message, line, column) = error_at(&format!("fun f()\n    give {source}\n"));
    assert_eq!(line, 2);
    // A expressão começa na coluna 10, depois de "    give ".
    (message, column - 9)
}
// pink-nav: end

// pink-nav: start
// pink-resume: Precedência e associatividade dos operadores, chamadas aninhadas e agrupamento por parênteses.
// pink-keywords: teste, precedência, associatividade, operador, comparação, not, negação, chamada
#[test]
fn multiplicacao_antes_da_soma() {
    assert_eq!(expression("a + b * c"), "(+ a (* b c))");
    assert_eq!(expression("a % b / c"), "(/ (% a b) c)");
}

#[test]
fn soma_e_subtracao_associam_a_esquerda() {
    assert_eq!(expression("a - b - c"), "(- (- a b) c)");
}

#[test]
fn negacao_prefixa_liga_mais_que_multiplicacao() {
    assert_eq!(expression("-a * b"), "(* (- a) b)");
    assert_eq!(expression("--a"), "(- (- a))");
}

#[test]
fn not_fica_abaixo_das_comparacoes() {
    assert_eq!(expression("not a == b"), "(not (== a b))");
    assert_eq!(expression("not a and b"), "(and (not a) b)");
}

#[test]
fn and_liga_mais_que_or() {
    assert_eq!(expression("a or b and c"), "(or a (and b c))");
    assert_eq!(expression("a or b or c"), "(or (or a b) c)");
}

#[test]
fn parenteses_agrupam() {
    assert_eq!(expression("(a + b) * c"), "(* (+ a b) c)");
    assert_eq!(expression("(a < b) < c"), "(< (< a b) c)");
}

#[test]
fn comparacoes_nao_se_encadeiam() {
    let (message, column) = expression_error("a < b < c");
    assert_eq!(column, 7);
    assert!(message.contains("não se encadeiam"), "{message}");
    let (_, column) = expression_error("a == b != c");
    assert_eq!(column, 8);
}

#[test]
fn not_nao_entra_como_operando_de_comparacao() {
    let (message, column) = expression_error("x == not y");
    assert_eq!(column, 6);
    assert_eq!(
        message,
        "'not' tem precedência menor que '==' e só pode ser operando dele entre parênteses"
    );
}

#[test]
fn chamadas_aninhadas_e_virgula_final() {
    assert_eq!(expression("f(1, g(2),)"), "(call f 1 (call g 2))");
    assert_eq!(expression("f()"), "(call f)");
}

#[test]
fn literais() {
    assert_eq!(expression("\"olá\""), "\"olá\"");
    assert_eq!(expression("true and false"), "(and true false)");
    assert_eq!(expression("0xFF"), "FF");
}

#[test]
fn minimo_de_i64_e_negacao_da_magnitude() {
    assert_eq!(
        expression("-9223372036854775808"),
        "(- 9223372036854775808)"
    );
}

#[test]
fn expressao_incompleta_e_erro() {
    let (message, _) = expression_error("a +");
    assert!(message.contains("encontrei o fim da linha"), "{message}");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Operando ausente: operador só binário no início, '+' prefixo, operador sem operando antes de um terminador, operadores em sequência; construções válidas e erros semânticos preservados.
// pink-keywords: teste, operando ausente, operador, + unário, terminador, fim do arquivo, not, precedência, semântica
/// A mensagem e a coluna do erro de sintaxe na linha 2 de `fun main()`.
fn main_error(line: &str) -> (String, usize) {
    let (message, at_line, column) = error_at(&format!("fun main()\n    {line}\n"));
    assert_eq!(at_line, 2, "{line}");
    (message, column)
}

#[test]
fn operador_so_binario_sem_operando_a_esquerda() {
    for (line, operator, column) in [
        ("say(* 2)", "*", 9),
        ("say(/ 2)", "/", 9),
        ("say(% 2)", "%", 9),
        ("say(== 2)", "==", 9),
        ("let x = and true", "and", 13),
        ("let x = or false", "or", 13),
        ("x = < 1", "<", 9),
        ("give != 1", "!=", 10),
        // Dois operadores seguidos: o segundo é o que não tem operando à esquerda.
        ("say(1 + * 2)", "*", 13),
        ("say(1 < < 2)", "<", 13),
    ] {
        assert_eq!(
            main_error(line),
            (
                format!("falta o operando à esquerda de '{operator}'"),
                column
            ),
            "{line}"
        );
    }
}

#[test]
fn mais_prefixo_nao_existe() {
    for (line, column) in [("let x = +1", 13), ("say(1 + + 2)", 13), ("say(-+1)", 10)] {
        assert_eq!(
            main_error(line),
            (
                "a Pinker não tem '+' unário: '+' precisa de um operando à esquerda".to_string(),
                column
            ),
            "{line}"
        );
    }
}

#[test]
fn operador_sem_operando_antes_de_um_terminador() {
    // A posição é a do token que mostra a ausência: a quebra, ')' ou ','.
    for (line, operator, found, column) in [
        ("let x = 1 +", "+", "o fim da linha", 16),
        ("say(1 *)", "*", "')'", 12),
        ("say(f(1 +, 2))", "+", "','", 14),
        ("let x = -", "-", "o fim da linha", 14),
        ("if not", "not", "o fim da linha", 11),
        ("let x = 1 and", "and", "o fim da linha", 18),
        ("say((1 ==))", "==", "')'", 14),
    ] {
        assert_eq!(
            main_error(line),
            (
                format!("falta o operando à direita de '{operator}', encontrei {found}"),
                column
            ),
            "{line}"
        );
    }
}

#[test]
fn operador_sem_operando_antes_do_fim_do_bloco_e_do_arquivo() {
    // O lexer sempre põe uma QUEBRA depois da última linha, então um .pk nunca leva o
    // parser a um DEDENT ou ao FIM logo depois de um operador. Um .t editado pode: os
    // tokens aqui são os do lexer sem a QUEBRA (e sem o DEDENT) que seguem o '+'.
    let tokens = tokenize("fun main()\n    give 1 +\n").unwrap();
    let plus = tokens.iter().position(|token| token.lexeme == "+").unwrap();
    for (removed, found) in [(1, "o fim do bloco"), (2, "o fim do arquivo")] {
        let mut edited = tokens.clone();
        edited.drain(plus + 1..plus + 1 + removed);
        assert!(matches!(
            edited[plus + 1].kind,
            TokenKind::Dedent | TokenKind::Eof
        ));
        let e = parse(&edited).expect_err("o parser deveria rejeitar");
        assert_eq!(
            e.message,
            format!("falta o operando à direita de '+', encontrei {found}")
        );
        assert_eq!(e.position, edited[plus + 1].position);
    }
}

#[test]
fn palavra_reservada_nao_e_apresentada_como_operador() {
    assert_eq!(
        main_error("let x = 1 + if"),
        (
            "esperava o operando à direita de '+', encontrei 'if'".to_string(),
            17
        )
    );
    // Sem operador antes, a mensagem continua a de sempre.
    assert_eq!(
        main_error("say(,)"),
        ("esperava uma expressão, encontrei ','".to_string(), 9)
    );
    assert_eq!(
        main_error("let x ="),
        (
            "esperava uma expressão, encontrei o fim da linha".to_string(),
            12
        )
    );
    assert_eq!(
        main_error("let x = - not true"),
        (
            "'not' tem precedência menor que '-' e só pode ser operando dele entre parênteses"
                .to_string(),
            15
        )
    );
}

#[test]
fn prefixos_e_vazios_validos_continuam_aceitos() {
    assert_eq!(expression("-1"), "(- 1)");
    assert_eq!(expression("1 + -2"), "(+ 1 (- 2))");
    assert_eq!(expression("1 - -2"), "(- 1 (- 2))");
    assert_eq!(expression("not not true"), "(not (not true))");
    assert_eq!(expression("a == (not b)"), "(== a (not b))");
    assert_eq!(expression("a and not b"), "(and a (not b))");
    assert_eq!(expression("\"a\" + \"b\""), "(+ \"a\" \"b\")");
}

#[test]
fn tipos_incompativeis_formam_uma_arvore() {
    // A forma é válida; a incompatibilidade é da análise semântica (tests/semantic.rs).
    assert_eq!(expression("1 + true"), "(+ 1 true)");
    assert_eq!(expression("-true"), "(- true)");
    assert_eq!(expression("not 1"), "(not 1)");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Instruções e funções: let, atribuição, else if, laços, give, parâmetros e blocos.
// pink-keywords: teste, instrução, let, atribuição, if, else, while, loop, give, função, parâmetro
#[test]
fn funcao_com_parametros_e_resultado() {
    let program = program("fun soma(a: int, b: i32,) -> int\n    give a + b\n");
    let function = &program.functions[0];
    assert_eq!(function.name.text, "soma");
    let parameters: Vec<(&str, &str)> = function
        .parameters
        .iter()
        .map(|p| (p.name.text.as_str(), p.type_name.text.as_str()))
        .collect();
    assert_eq!(parameters, [("a", "int"), ("b", "i32")]);
    assert_eq!(function.result.as_ref().unwrap().text, "int");
}

#[test]
fn let_mut_com_tipo_e_atribuicao() {
    let program = program("fun main()\n    let mut x: u8 = 1\n    x = x + 1\n");
    let statements = &program.functions[0].body.statements;
    let StatementKind::Let {
        mutable,
        name,
        type_name,
        value,
    } = &statements[0].kind
    else {
        panic!("esperava let: {:?}", statements[0].kind);
    };
    assert!(mutable);
    assert_eq!(name.text, "x");
    assert_eq!(type_name.as_ref().unwrap().text, "u8");
    assert_eq!(sexpr(value), "1");
    let StatementKind::Assign { target, value } = &statements[1].kind else {
        panic!("esperava atribuição: {:?}", statements[1].kind);
    };
    assert_eq!(target.text, "x");
    assert_eq!(sexpr(value), "(+ x 1)");
}

#[test]
fn else_if_encadeia_sem_novo_nivel() {
    let source = "fun f(x: int)\n    if x > 0\n        say(1)\n    else if x < 0\n        say(2)\n    else\n        say(3)\n";
    let program = program(source);
    let StatementKind::If { else_branch, .. } = &program.functions[0].body.statements[0].kind
    else {
        panic!("esperava if");
    };
    let else_branch = else_branch.as_ref().unwrap();
    assert_eq!(else_branch.position, Position { line: 4, column: 5 });
    let ElseBranch::If(inner) = &else_branch.branch else {
        panic!("esperava else if");
    };
    let StatementKind::If {
        condition,
        else_branch,
        ..
    } = &inner.kind
    else {
        panic!("esperava if interno");
    };
    assert_eq!(sexpr(condition), "(< x 0)");
    assert!(matches!(
        else_branch.as_ref().unwrap().branch,
        ElseBranch::Block(_)
    ));
}

#[test]
fn lacos_e_give_sem_valor() {
    let source = "fun f()\n    while true\n        continue\n    loop\n        break\n    give\n";
    let statements = &program(source).functions[0].body.statements;
    assert!(matches!(statements[0].kind, StatementKind::While { .. }));
    assert!(matches!(statements[1].kind, StatementKind::Loop { .. }));
    assert!(matches!(
        statements[2].kind,
        StatementKind::Give { value: None }
    ));
}

#[test]
fn break_fora_de_laco_e_sintaticamente_valido() {
    // A rejeição de break fora de laço pertence à análise semântica.
    let statements = &program("fun main()\n    break\n").functions[0]
        .body
        .statements;
    assert!(matches!(statements[0].kind, StatementKind::Break));
}

#[test]
fn arquivo_vazio_e_um_programa_sem_funcoes() {
    assert!(program("").functions.is_empty());
}
// pink-nav: end

// pink-nav: start
// pink-resume: Rejeições sintáticas: indentação inesperada, bloco ausente, else sem if, instrução inválida.
// pink-keywords: teste, erro, indentação inesperada, bloco, else, instrução, nível do arquivo
#[test]
fn indentacao_inesperada_e_erro() {
    let (message, line, column) = error_at("fun main()\n    let value = 1\n        say(value)\n");
    assert_eq!((line, column), (3, 9));
    assert_eq!(message, "indentação inesperada");
}

#[test]
fn funcao_sem_bloco_e_erro() {
    let (message, line, column) = error_at("fun main()\nsay(1)\n");
    assert_eq!((line, column), (2, 1));
    assert!(message.contains("bloco indentado"), "{message}");
}

#[test]
fn else_sem_if_e_erro() {
    let (message, line, column) = error_at("fun main()\n    else\n        say(1)\n");
    assert_eq!((line, column), (2, 5));
    assert!(message.contains("sem 'if'"), "{message}");
}

#[test]
fn so_chamadas_podem_ser_instrucoes() {
    let (message, line, column) = error_at("fun main()\n    a + b\n");
    assert_eq!((line, column), (2, 7));
    assert!(message.contains("'=' ou '('"), "{message}");
    let (message, line, column) = error_at("fun main()\n    f(x) + 1\n");
    assert_eq!((line, column), (2, 10));
    assert!(message.contains("descartado"), "{message}");
}

#[test]
fn atribuicao_nao_e_expressao() {
    let (_, line, column) = error_at("fun main()\n    if x = 1\n        say(x)\n");
    assert_eq!((line, column), (2, 10));
}

#[test]
fn nivel_do_arquivo_so_aceita_funcoes() {
    let (message, line, column) = error_at("let x = 1\n");
    assert_eq!((line, column), (1, 1));
    assert!(message.contains("'fun'"), "{message}");
}

#[test]
fn parametro_sem_tipo_e_erro() {
    let (message, line, column) = error_at("fun f(a int)\n    give\n");
    assert_eq!((line, column), (1, 9));
    assert!(message.contains("':'"), "{message}");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Saída de --emit=ast: a árvore indentada completa de um programa pequeno, com posições.
// pink-keywords: teste, emit, ast, render, árvore
#[test]
fn render_mostra_a_arvore_com_posicoes() {
    let source =
        "fun main()\n    let mut n = -1\n    if n < 0 and not false\n        say(\"negativo\")\n";
    let expected = "\
# pinker ast 1 · fonte: teste.pk
#POSIÇÃO NÓ
1:1      Função main@1:5
2:5        Bloco
2:5          Let mut n@2:13
2:17           Unária -
2:18             Inteiro 1 (base 10)
3:5          If
3:14           Binária and
3:10             Binária <
3:8                Nome n
3:12               Inteiro 0 (base 10)
3:18             Unária not
3:22               Lógico false
4:9            Bloco
4:9              Chamada say
4:13               Texto \"negativo\"
";
    assert_eq!(ast::render(&program(source), "teste.pk"), expected);
}
// pink-nav: end
