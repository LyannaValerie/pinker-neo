//! Testes de execução: saída de say, aritmética, falhas de execução, fluxo e ordem. Cada
//! programa roda nos dois motores, a máquina virtual e o interpretador de árvore, que
//! precisam dar a mesma saída e o mesmo diagnóstico.

use std::io::{self, Write};

use pinker::bytecode;
use pinker::checked::CheckedProgram;
use pinker::diagnostic::Diagnostic;
use pinker::lexer::tokenize;
use pinker::lower::lower;
use pinker::parser::parse;
use pinker::semantic::check;
use pinker::{interpreter, vm};

// pink-nav: start
// pink-resume: Auxiliares: executam um programa nos dois motores, exigem o mesmo resultado e devolvem a saída escrita e a falha, se houver.
// pink-keywords: auxiliar, helper, execute, output, falha, paridade, vm, interpretador
fn checked(source: &str) -> CheckedProgram {
    let tokens = tokenize(source).unwrap_or_else(|e| panic!("erro léxico em {source:?}: {e:?}"));
    let program = parse(&tokens).unwrap_or_else(|e| panic!("erro de sintaxe em {source:?}: {e:?}"));
    check(&program).unwrap_or_else(|e| panic!("erro semântico em {source:?}: {e:?}"))
}

/// Executa o programa no interpretador de árvore e na máquina virtual (com o bytecode
/// conferido pelo verificador), exige a mesma saída e o mesmo resultado, e os devolve.
fn execute(source: &str) -> (String, Result<(), Diagnostic>) {
    let checked = checked(source);
    let mut by_tree = Vec::new();
    let tree_result = interpreter::run(&checked, &mut by_tree);

    let lowered = lower(&checked);
    bytecode::verify(&lowered)
        .unwrap_or_else(|e| panic!("bytecode recusado para {source:?}: {e:?}"));
    let mut by_vm = Vec::new();
    let vm_result = vm::run(&lowered, &mut by_vm);

    assert_eq!(by_vm, by_tree, "saídas diferentes para {source:?}");
    assert_eq!(
        vm_result, tree_result,
        "resultados diferentes para {source:?}"
    );
    (String::from_utf8(by_vm).unwrap(), vm_result)
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

// pink-nav: start
// pink-resume: Concatenação de textos: literais, variáveis e resultados; vazios; Unicode e escapes; encadeamento; ordem e avaliação única; soma de inteiros intacta; textos dinâmicos depois do bloco e da função; cópia e substituição.
// pink-keywords: teste, concatenação, texto, +, vazio, unicode, escape, ordem, avaliação única, texto dinâmico, cópia
#[test]
fn concatena_literais_variaveis_e_resultados_de_funcoes() {
    assert_eq!(
        output(
            "fun saudacao(nome: text) -> text\n    give \"Olá, \" + nome\n\nfun main()\n    say(saudacao(\"Pinker\"))\n    let a = \"Pin\"\n    let b = \"ker\"\n    say(a + b)\n    say(saudacao(a) + \"!\")\n"
        ),
        "Olá, Pinker\nPinker\nOlá, Pin!\n"
    );
}

#[test]
fn texto_vazio_em_cada_lado() {
    assert_eq!(
        main_output(&[
            "let vazio = \"\"",
            "say(\"\" + \"x\")",
            "say(\"x\" + \"\")",
            "say(vazio + vazio == \"\")",
            "say(\"[\" + (\"\" + \"\") + \"]\")",
        ]),
        "x\nx\ntrue\n[]\n"
    );
}

#[test]
fn unicode_e_escapes_sao_preservados_sem_normalizacao() {
    // "é" composto (U+00E9) e decomposto (e + U+0301) continuam diferentes: a
    // concatenação copia os bytes, sem normalizar.
    let source = main_with(&[
        "say(\"ação\" + \" 🌸 \" + \"日本\")",
        "say(\"linha\\n\" + \"\\t\\\"aspas\\\"\")",
        "say(\"\u{e9}\" + \"|\" + \"e\u{301}\")",
        "say(\"\u{e9}\" == \"e\u{301}\")",
    ]);
    assert_eq!(
        output(&source),
        "ação 🌸 日本\nlinha\n\t\"aspas\"\n\u{e9}|e\u{301}\nfalse\n"
    );
}

#[test]
fn encadeamento_associa_pela_esquerda_e_precedencia_se_mantem() {
    assert_eq!(
        main_output(&[
            "say(\"a\" + \"b\" + \"c\" + \"d\")",
            "say(\"a\" + \"b\" == \"ab\")",
            "say(\"a\" + (\"b\" + \"c\"))",
        ]),
        "abcd\ntrue\nabc\n"
    );
}

#[test]
fn operandos_avaliados_uma_vez_da_esquerda_para_a_direita() {
    assert_eq!(
        output(
            "fun marca(t: text) -> text\n    say(t)\n    give t\n\nfun main()\n    say(marca(\"1\") + marca(\"2\") + marca(\"3\"))\n"
        ),
        "1\n2\n3\n123\n"
    );
}

#[test]
fn soma_de_inteiros_e_seus_erros_se_mantem() {
    assert_eq!(
        main_output(&["say(2 + 3)", "let x: u8 = 250", "say(x + 5)"]),
        "5\n255\n"
    );
    let (output, message, line, column) =
        failure(&["let x: u8 = 255", "say(\"antes\")", "say(x + 1)"]);
    assert_eq!(output, "antes\n");
    assert_eq!(message, "overflow: 255 + 1 = 256 não cabe em u8");
    assert_eq!((line, column), (4, 11));
}

#[test]
fn texto_dinamico_sobrevive_ao_fim_do_bloco_e_da_funcao() {
    assert_eq!(
        output(
            "fun junta(a: text, b: text) -> text\n    if true\n        let r = a + b\n        give r\n    give a\n\nfun main()\n    let mut guardado = \"\"\n    if true\n        let t = junta(\"x\", \"y\") + \"z\"\n        guardado = t\n    say(guardado)\n    let mut i = 0\n    let mut acumulado = \"\"\n    while i < 3\n        let parte = \"-\" + \"o\"\n        acumulado = acumulado + parte\n        i = i + 1\n    say(acumulado)\n"
        ),
        "xyz\n-o-o-o\n"
    );
}

#[test]
fn copia_e_substituicao_nao_invalidam_outros_valores() {
    assert_eq!(
        main_output(&[
            "let a = \"p\" + \"q\"",
            "let mut b = a",
            "b = b + \"!\"",
            "let mut c = b",
            "b = \"novo\"",
            "say(a)",
            "say(b)",
            "say(c)",
        ]),
        "pq\nnovo\npq!\n"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Falhas da saída: escrita recusada falha no say; descarga recusada no fim falha mesmo sem erro do programa.
// pink-keywords: teste, saída, escrita, flush, descarga, write, falha de escrita
/// Uma saída que recusa a escrita (`write`) ou a descarga (`flush`).
struct FailingOutput {
    fails_on_write: bool,
}

impl Write for FailingOutput {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.fails_on_write {
            return Err(io::Error::other("escrita recusada"));
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("descarga recusada"))
    }
}

/// A falha nos dois motores, que precisa ser a mesma.
fn run_with(source: &str, fails_on_write: bool) -> Diagnostic {
    let checked = checked(source);
    let by_tree = interpreter::run(&checked, &mut FailingOutput { fails_on_write })
        .expect_err("a execução deveria falhar");
    let by_vm = vm::run(&lower(&checked), &mut FailingOutput { fails_on_write })
        .expect_err("a execução deveria falhar");
    assert_eq!(by_vm, by_tree);
    by_vm
}

#[test]
fn escrita_recusada_falha_no_say() {
    let e = run_with("fun main()\n    say(1)\n", true);
    assert_eq!(
        e.message,
        "não foi possível escrever a saída: escrita recusada"
    );
    assert_eq!((e.position.line, e.position.column), (2, 5));
}

#[test]
fn descarga_recusada_no_fim_e_falha_de_execucao() {
    // Cada say é aceito, mas a saída não chega ao destino: a execução não pode terminar
    // com sucesso.
    let e = run_with("fun main()\n    say(1)\n    say(2)\n", false);
    assert_eq!(
        e.message,
        "não foi possível escrever a saída: descarga recusada"
    );
    assert_eq!((e.position.line, e.position.column), (1, 1));
}
// pink-nav: end

// pink-nav: start
// pink-resume: Os valores e o limite de chamadas continuam acessíveis pelo caminho antigo do interpretador, além do runtime.
// pink-keywords: teste, runtime, reexportação, caminho antigo, value, limite de chamadas
#[test]
fn caminho_antigo_do_interpretador_continua_valendo() {
    assert_eq!(
        pinker::interpreter::MAX_CALL_DEPTH,
        pinker::runtime::MAX_CALL_DEPTH
    );
    let value: pinker::runtime::Value = pinker::interpreter::Value::Bool(true);
    assert_eq!(value.to_string(), "true");
}
// pink-nav: end
