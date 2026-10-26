//! Testes do interpretador: saída de say, aritmética, falhas de execução, fluxo e ordem.

use pinker::diagnostic::Diagnostic;
use pinker::interpreter::run;
use pinker::lexer::tokenize;
use pinker::parser::parse;
use pinker::semantic::check;

// pink-nav: start
// pink-resume: Auxiliares: executam um programa e devolvem a saída escrita e a falha de execução, se houver.
// pink-keywords: auxiliar, helper, execute, output, falha
/// Executa o programa e devolve o que `say` escreveu e o resultado da execução.
fn execute(source: &str) -> (String, Result<(), Diagnostic>) {
    let tokens = tokenize(source).unwrap_or_else(|e| panic!("erro léxico em {source:?}: {e:?}"));
    let program = parse(&tokens).unwrap_or_else(|e| panic!("erro de sintaxe em {source:?}: {e:?}"));
    let checked = check(&program).unwrap_or_else(|e| panic!("erro semântico em {source:?}: {e:?}"));
    let mut output = Vec::new();
    let result = run(&checked, &mut output);
    (String::from_utf8(output).unwrap(), result)
}

/// A saída de um programa que deve terminar normalmente.
fn output(source: &str) -> String {
    let (output, result) = execute(source);
    result.unwrap_or_else(|e| panic!("falha de execução em {source:?}: {e:?}"));
    output
}

/// A saída de uma `main` com as linhas dadas.
fn main_output(lines: &[&str]) -> String {
    output(&main_with(lines))
}

fn main_with(lines: &[&str]) -> String {
    let mut source = String::from("fun main()\n");
    for line in lines {
        source.push_str("    ");
        source.push_str(line);
        source.push('\n');
    }
    source
}

/// A saída escrita antes da falha, a mensagem e a posição da falha.
fn failure(lines: &[&str]) -> (String, String, usize, usize) {
    let (output, result) = execute(&main_with(lines));
    let e = result.expect_err("a execução deveria falhar");
    (output, e.message, e.position.line, e.position.column)
}
// pink-nav: end

// pink-nav: start
// pink-resume: say escreve inteiros, bool e texto com quebra de linha; aritmética segue o contrato de divisão e resto.
// pink-keywords: teste, say, saída, aritmética, divisão, resto, truncamento
#[test]
fn say_escreve_cada_tipo_com_quebra_de_linha() {
    assert_eq!(
        main_output(&[
            "say(42)",
            "say(-7)",
            "say(true)",
            "say(\"olá\\tPinker\")",
            "let x: u64 = 18446744073709551615",
            "say(x)"
        ]),
        "42\n-7\ntrue\nolá\tPinker\n18446744073709551615\n"
    );
}

#[test]
fn divisao_trunca_em_direcao_a_zero_e_resto_segue_o_dividendo() {
    assert_eq!(
        main_output(&[
            "let a = 7",
            "let b = -7",
            "let c = 3",
            "say(a / c)",
            "say(b / c)",
            "say(b % c)",
            "say(a % -c)",
        ]),
        "2\n-2\n-1\n1\n"
    );
}

#[test]
fn minimo_por_menos_um() {
    let lines = ["let m = -9223372036854775808", "let d = -1"];
    assert_eq!(main_output(&[lines[0], lines[1], "say(m % d)"]), "0\n");
    let (_, message, ..) = failure(&[lines[0], lines[1], "say(m / d)"]);
    assert!(message.starts_with("overflow"), "{message}");
}

#[test]
fn comparacoes_e_igualdade() {
    assert_eq!(
        main_output(&[
            "let a = 2",
            "say(a < 3)",
            "say(a >= 3)",
            "say(\"x\" == \"x\")",
            "say(true != false)",
        ]),
        "true\nfalse\ntrue\ntrue\n"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Falhas de execução: overflow, negação, divisão por zero e conversão, com posição e saída anterior preservada.
// pink-keywords: teste, falha de execução, overflow, divisão por zero, morph, conversão, posição
#[test]
fn overflow_em_execucao_falha_no_operador() {
    let (output, message, line, column) = failure(&[
        "let mut x: u8 = 254",
        "x = x + 1",
        "say(x)",
        "x = x + 1",
        "say(x)",
    ]);
    assert_eq!(output, "255\n");
    assert_eq!(message, "overflow: 255 + 1 = 256 não cabe em u8");
    assert_eq!((line, column), (5, 11));
}

#[test]
fn negacao_do_minimo_falha() {
    let (_, message, ..) = failure(&["let x: i8 = -128", "say(-x)"]);
    assert_eq!(message, "overflow: -(-128) não cabe em i8");
}

#[test]
fn divisao_por_zero_em_execucao_falha() {
    let (_, message, line, column) = failure(&["let zero = 0", "say(1 % zero)"]);
    assert_eq!(message, "divisão por zero");
    assert_eq!((line, column), (3, 11));
}

#[test]
fn morph_confere_a_faixa_em_execucao() {
    assert_eq!(
        main_output(&[
            "let a: i32 = 7",
            "let b: i64 = morph(a)",
            "let c: u64 = 5",
            "let d: i64 = morph(c)",
            "say(b + d)"
        ]),
        "12\n"
    );
    let (_, message, line, column) = failure(&["let a = 300", "let b: u8 = morph(a)"]);
    assert_eq!(message, "conversão fora da faixa: 300 não cabe em u8");
    assert_eq!((line, column), (3, 17));
    let (_, message, ..) = failure(&["let a: i8 = -1", "let b: u8 = morph(a)"]);
    assert_eq!(message, "conversão fora da faixa: -1 não cabe em u8");
}

#[test]
fn falha_encerra_o_programa_na_hora() {
    let (output, ..) = failure(&["say(1)", "let z = 0", "say(1 / z)", "say(2)"]);
    assert_eq!(output, "1\n");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Fluxo: if/else, laços com break e continue, break de laço interno, give dentro de laço e recursão.
// pink-keywords: teste, fluxo, if, else, while, loop, break, continue, give, recursão
#[test]
fn if_else_if_else() {
    let source = "fun sinal(x: int) -> text\n    if x > 0\n        give \"positivo\"\n    else if x < 0\n        give \"negativo\"\n    else\n        give \"zero\"\n\nfun main()\n    say(sinal(5))\n    say(sinal(-5))\n    say(sinal(0))\n";
    assert_eq!(output(source), "positivo\nnegativo\nzero\n");
}

#[test]
fn while_com_continue_e_break() {
    assert_eq!(
        main_output(&[
            "let mut i = 0",
            "while true",
            "    i = i + 1",
            "    if i % 2 == 0",
            "        continue",
            "    if i > 7",
            "        break",
            "    say(i)",
        ]),
        "1\n3\n5\n7\n"
    );
}

#[test]
fn break_encerra_so_o_laco_interno() {
    assert_eq!(
        main_output(&[
            "let mut externo = 0",
            "loop",
            "    externo = externo + 1",
            "    loop",
            "        break",
            "    say(externo)",
            "    if externo == 2",
            "        break",
        ]),
        "1\n2\n"
    );
}

#[test]
fn give_dentro_de_laco_encerra_a_funcao() {
    let source = "fun primeiro_maior(limite: int) -> int\n    let mut n = 0\n    loop\n        n = n + 3\n        if n > limite\n            give n\n\nfun main()\n    say(primeiro_maior(10))\n";
    assert_eq!(output(source), "12\n");
}

#[test]
fn give_sem_valor_encerra_antes() {
    let source = "fun f(x: bool)\n    if x\n        give\n    say(\"chegou\")\n\nfun main()\n    f(true)\n    f(false)\n";
    assert_eq!(output(source), "chegou\n");
}

#[test]
fn recursao() {
    let source = "fun fib(n: int) -> int\n    if n < 2\n        give n\n    give fib(n - 1) + fib(n - 2)\n\nfun main()\n    say(fib(20))\n";
    assert_eq!(output(source), "6765\n");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Ordem de avaliação da esquerda para a direita, curto-circuito, sombreamento e textos compartilhados.
// pink-keywords: teste, ordem de avaliação, curto-circuito, sombreamento, texto, variável
const NOISY: &str = "fun a() -> int\n    say(\"a\")\n    give 1\n\nfun b() -> int\n    say(\"b\")\n    give 2\n\nfun t() -> bool\n    say(\"t\")\n    give true\n\nfun soma(x: int, y: int) -> int\n    give x + y\n\n";

#[test]
fn operandos_e_argumentos_da_esquerda_para_a_direita() {
    assert_eq!(
        output(&format!("{NOISY}fun main()\n    say(a() + b())\n")),
        "a\nb\n3\n"
    );
    assert_eq!(
        output(&format!("{NOISY}fun main()\n    say(soma(b(), a()))\n")),
        "b\na\n3\n"
    );
}

#[test]
fn curto_circuito_nao_avalia_o_lado_direito() {
    let main = "fun main()\n    say(false and t())\n    say(true or t())\n    say(true and t())\n";
    assert_eq!(output(&format!("{NOISY}{main}")), "false\ntrue\nt\ntrue\n");
}

#[test]
fn sombreamento_nao_altera_a_variavel_externa() {
    assert_eq!(
        main_output(&[
            "let x = 1",
            "if true",
            "    let x = 2",
            "    say(x)",
            "say(x)"
        ]),
        "2\n1\n"
    );
}

#[test]
fn let_dentro_de_laco_comeca_de_novo_a_cada_volta() {
    assert_eq!(
        main_output(&[
            "let mut i = 0",
            "while i < 3",
            "    let dobro = i * 2",
            "    say(dobro)",
            "    i = i + 1",
        ]),
        "0\n2\n4\n"
    );
}

#[test]
fn texto_atribuido_nao_altera_o_original() {
    assert_eq!(
        main_output(&[
            "let first = \"Pinker\"",
            "let mut second = first",
            "second = \"Outra\"",
            "say(first)",
            "say(second)"
        ]),
        "Pinker\nOutra\n"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Fronteiras de execução: multiplicação que não cabe nem em 128 bits e limite contado por chamadas ativas.
// pink-keywords: teste, checked_mul, u64, parâmetro, limite, chamadas ativas
#[test]
fn produto_de_u64_maximos_falha_sem_panico() {
    // Os operandos chegam por parâmetro: a conta acontece na execução, não na análise.
    let source = "fun produto(a: u64, b: u64) -> u64\n    give a * b\n\nfun main()\n    say(produto(18446744073709551615, 18446744073709551615))\n";
    let (_, result) = execute(source);
    let e = result.expect_err("a execução deveria falhar");
    assert_eq!(
        e.message,
        "overflow: o resultado de 18446744073709551615 * 18446744073709551615 não cabe em u64"
    );
    assert_eq!((e.position.line, e.position.column), (2, 12));
}

#[test]
fn limite_conta_so_as_chamadas_ativas() {
    let source = "fun um() -> int\n    give 1\n\nfun main()\n    let mut i = 0\n    let mut total = 0\n    while i < 30000\n        total = total + um()\n        i = i + 1\n    say(total)\n";
    assert_eq!(output(source), "30000\n");
}

// pink-nav: end
