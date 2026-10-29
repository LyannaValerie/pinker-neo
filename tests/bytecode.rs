//! Testes do bytecode: a tradução da árvore verificada, o arquivo .b (ida e volta e erros
//! de formato) e o verificador, que recusa bytecode adulterado antes da máquina virtual.

use pinker::artifact::{self, Format, FormatError};
use pinker::bytecode::{self, BytecodeProgram};
use pinker::lexer::tokenize;
use pinker::lower::lower;
use pinker::parser::parse;
use pinker::semantic::check;
use pinker::vm;

// pink-nav: start
// pink-resume: Auxiliares: traduz um programa, grava o .b e devolve a falha do verificador ou da leitura depois de uma edição.
// pink-keywords: auxiliar, helper, lower, render, edição, verificador, erro de formato
/// Usa todas as construções da primeira versão, com laços aninhados, variáveis declaradas
/// dentro de laços, break e continue que saem de vários blocos, curto-circuito e código
/// depois de give.
const COMPLETE: &str = r#"fun fatorial(n: u64) -> u64
    if n <= 1
        give 1
    give n * fatorial(n - 1)

fun classifica(x: int) -> text
    if x > 0
        give "positivo"
    else if x < 0
        give "negativo"
    else
        give "zero: \"nada\""

fun conta(limite: i32)
    let mut i: i32 = 0
    while true
        let passo: i32 = 1
        i = i + passo
        if i % 2 == 0
            let par = true
            continue
        if i > limite
            if true
                let saindo = "sim"
                break
        say(i)
    loop
        let mut j = 0
        loop
            j = j + 1
            if j == 3
                break
        break
    give
    say("nunca")

fun main()
    let mascara: u8 = 0xFF
    let largo: i64 = morph(mascara)
    say(fatorial(10))
    say(classifica(-3))
    say(classifica(0))
    say(not (largo == 255) or false and true)
    say(-largo)
    say("a" != "b")
    conta(5)
    let x = 1
    if true
        let x = "sombra"
        say(x)
    say(x)
"#;

fn lowered(source: &str) -> BytecodeProgram {
    let program = check(&parse(&tokenize(source).unwrap()).unwrap()).unwrap();
    let lowered = lower(&program);
    bytecode::verify(&lowered).unwrap_or_else(|e| panic!("bytecode recusado: {e:?}"));
    lowered
}

fn rendered(source: &str) -> String {
    bytecode::render(&lowered(source), "c.pk")
}

/// Grava o programa como .b, aplica a edição e devolve a mensagem do verificador.
fn tampered(source: &str, from: &str, to: &str) -> String {
    let content = rendered(source);
    assert!(content.contains(from), "{content}");
    let program = bytecode::read(&content.replacen(from, to, 1)).unwrap();
    bytecode::verify(&program)
        .expect_err("o verificador deveria recusar")
        .message
}

/// Grava o programa como .b, aplica a edição e devolve o erro de formato.
fn malformed(source: &str, from: &str, to: &str) -> (usize, String) {
    let content = rendered(source);
    assert!(content.contains(from), "{content}");
    let e: FormatError =
        bytecode::read(&content.replacen(from, to, 1)).expect_err("a leitura deveria falhar");
    (e.line, e.message)
}
// pink-nav: end

// pink-nav: start
// pink-resume: Ida e volta do .b: gravar e ler devolve o mesmo bytecode, que executa igual.
// pink-keywords: teste, ida e volta, round trip, bytecode, arquivo .b, execução
#[test]
fn bytecode_ida_e_volta() {
    let program = lowered(COMPLETE);
    let content = bytecode::render(&program, "c.pk");
    assert_eq!(
        artifact::detect(&content).unwrap(),
        Some((Format::Bytecode, "c.pk".to_string()))
    );
    let read = bytecode::read(&content).unwrap();
    assert_eq!(read, program);
    bytecode::verify(&read).unwrap();
    let mut output = Vec::new();
    vm::run(&read, &mut output).unwrap();
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "3628800\nnegativo\nzero: \"nada\"\nfalse\n-255\ntrue\n1\n3\n5\nsombra\n1\n"
    );
}

#[test]
fn bytecode_mostra_a_tabela_e_as_instrucoes() {
    let content =
        rendered("fun dobro(n: u8) -> u8\n    give n * 2\n\nfun main()\n    say(dobro(3))\n");
    let expected = "\
# pinker bytecode 1 · fonte: c.pk
[funções]
#NOME  DECLARAÇÃO  ASSINATURA
dobro  1:5         (u8) -> u8
main   4:5         ()

[variáveis dobro]
#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE
0   n     u8    1:11        parâmetro

[variáveis main]
#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE

[código dobro 1:1]
#N  POSIÇÃO  INSTRUÇÃO
0   2:10     load n#0
1   2:14     const.u8 2
2   2:12     mul.u8
3   2:5      give

[código main 4:1]
#N  POSIÇÃO  INSTRUÇÃO
0   5:15     const.u8 3
1   5:9      call dobro
2   5:5      say.u8
3   4:1      give
";
    assert_eq!(content, expected);
}
// pink-nav: end

// pink-nav: start
// pink-resume: Tradução: código depois de give não é traduzido, e break e continue esvaziam as variáveis dos blocos de que saem antes de saltar.
// pink-keywords: teste, lower, tradução, código morto, give, break, continue, clear, curto-circuito
#[test]
fn codigo_depois_de_give_nao_e_traduzido() {
    let program = lowered("fun main()\n    give\n    say(1)\n");
    let main = &program.functions[program.main];
    assert_eq!(main.code.len(), 1, "{:?}", main.code);
}

#[test]
fn break_e_continue_esvaziam_as_variaveis_antes_de_saltar() {
    let content = rendered(
        "fun main()\n    loop\n        let a = 1\n        if true\n            let b = 2\n            break\n        continue\n",
    );
    // O break sai de dois blocos: esvazia b e a, nessa ordem, e salta.
    assert!(
        content.contains("6   5:13     clear b#1\n7   3:9      clear a#0\n8   6:13     jump 11"),
        "{content}"
    );
    // O continue sai do corpo do laço: esvazia a e volta ao início.
    assert!(
        content.contains("9   3:9      clear a#0\n10  7:9      jump 0"),
        "{content}"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Verificador: bytecode editado à mão com tipos, variáveis, saltos, pilhas ou fim do código incoerentes é recusado antes da execução.
// pink-keywords: teste, verificador, bytecode inconsistente, pilha, tipo, variável vazia, imutável, salto, fim do código, give
#[test]
fn verificador_recusa_tipo_errado_na_pilha() {
    let source = "fun main()\n    let a = 1\n    say(a + 2)\n";
    let message = tampered(source, "add.i64", "add.u8");
    assert!(
        message
            .contains("bytecode inconsistente: instrução 4: esperava u8 na pilha, encontrei i64"),
        "{message}"
    );
}

#[test]
fn verificador_recusa_variavel_lida_vazia() {
    let source = "fun main()\n    let a = 1\n    say(a)\n";
    let message = tampered(source, "store a#0", "pop");
    assert!(
        message.contains("a variável #0 pode estar vazia aqui"),
        "{message}"
    );
}

#[test]
fn verificador_recusa_imutavel_guardada_de_novo() {
    let source = "fun main()\n    let a = 1\n    say(a)\n";
    let message = tampered(source, "say.i64", "store a#0");
    assert!(
        message.contains("a variável imutável #0 pode já ter um valor"),
        "{message}"
    );
    // Uma variável mutável pode receber de novo.
    let source = "fun main()\n    let mut a = 1\n    say(a)\n";
    let program = bytecode::read(&rendered(source).replacen("say.i64", "store a#0", 1)).unwrap();
    bytecode::verify(&program).unwrap();
}

#[test]
fn imutavel_guarda_de_novo_depois_de_clear() {
    // É o que um let dentro de um laço faz a cada volta: o clear encerra a vida da
    // variável, e o store seguinte começa outra.
    let source = "fun main()\n    let mut i = 0\n    while i < 2\n        let a = i\n        say(a)\n        i = i + 1\n";
    let content = rendered(source);
    assert!(content.contains("clear a#1"), "{content}");
    // Sem os blocos no bytecode, um clear acrescentado à mão tem o mesmo efeito: o
    // programa continua coerente, como se x tivesse sido declarada de novo.
    let edited = "\
# pinker bytecode 1 · fonte: a.pk
[funções]
main  1:5  ()
[variáveis main]
0  x  i64  2:9  let
[código main 1:1]
0  2:13  const.i64 1
1  2:5   store x#0
2  3:5   clear x#0
3  3:13  const.i64 2
4  3:5   store x#0
5  4:9   load x#0
6  4:5   say.i64
7  2:5   clear x#0
8  1:1   give
";
    let program = bytecode::read(edited).unwrap();
    bytecode::verify(&program).unwrap();
    let mut output = Vec::new();
    vm::run(&program, &mut output).unwrap();
    assert_eq!(output, b"2\n");
    // Sem o clear (no lugar dele, um salto para a instrução seguinte), o segundo store
    // é recusado.
    let program = bytecode::read(&edited.replacen("clear x#0", "jump 3", 1)).unwrap();
    let message = bytecode::verify(&program).unwrap_err().message;
    assert!(
        message.contains("instrução 4: a variável imutável #0 pode já ter um valor"),
        "{message}"
    );
}

#[test]
fn verificador_recusa_variavel_esvaziada_num_caminho() {
    // Num caminho a variável é esvaziada, no outro não: depois do encontro, ela pode estar
    // vazia.
    let source = "fun main()\n    let mut a = 1\n    if a == 1\n        a = 2\n    say(a)\n";
    let message = tampered(source, "store a#0\n", "clear a#0\n");
    assert!(message.contains("pode estar vazia"), "{message}");
}

#[test]
fn verificador_recusa_salto_para_fora_do_codigo() {
    let source = "fun main()\n    if true\n        say(1)\n";
    let message = tampered(source, "jump_false 4", "jump_false 99");
    assert!(
        message.contains("salto para a instrução 99, fora do código"),
        "{message}"
    );
}

#[test]
fn verificador_recusa_pilhas_diferentes_no_encontro() {
    let source = "fun main()\n    if true\n        say(\"a\")\n    else\n        say(\"b\")\n";
    // O ramo do else chega ao fim do if depois do ramo do then, e com outra pilha.
    let message = tampered(source, "5:9      say.text", "5:9      const.bool true");
    assert!(
        message.contains("alcançada com pilhas diferentes"),
        "{message}"
    );
}

#[test]
fn verificador_recusa_execucao_que_passa_do_fim() {
    let source = "fun main()\n    let x = 1\n    say(x)\n";
    let content = rendered(source);
    let last = content.trim_end().rsplit_once('\n').unwrap().1;
    assert!(last.ends_with("give"), "{content}");
    let message = tampered(source, last, &last.replace("give", "clear x#0"));
    assert!(
        message.contains("pode passar da última instrução"),
        "{message}"
    );
}

#[test]
fn verificador_recusa_give_com_a_pilha_errada() {
    let source = "fun f() -> int\n    give 1\n\nfun main()\n    say(f())\n";
    let message = tampered(source, "const.i64 1", "const.bool true");
    assert!(
        message.contains("give com a pilha [bool]; esperava [i64]"),
        "{message}"
    );
    let message = tampered(source, "say.i64", "const.i64 2");
    assert!(
        message.contains("give com a pilha [i64, i64]; esperava []"),
        "{message}"
    );
}

#[test]
fn verificador_recusa_constante_fora_da_faixa_e_neg_sem_sinal() {
    let source = "fun main()\n    let x: u8 = 1\n    say(x)\n";
    let message = tampered(source, "const.u8 1", "const.u8 300");
    assert!(message.contains("o valor 300 não cabe em u8"), "{message}");
    let source = "fun main()\n    let x: i8 = 1\n    say(-x)\n";
    let message = tampered(source, "neg.i8", "neg.u8");
    assert!(
        message.contains("neg exige um inteiro com sinal"),
        "{message}"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Erros de formato do .b: instrução, tipos, operandos, nomes, numeração e seções de código.
// pink-keywords: teste, erro de formato, arquivo .b, instrução, mnemônico, operando, seção, numeração
#[test]
fn instrucoes_malformadas() {
    let source = "fun main()\n    let a = 1\n    say(a + 2)\n";
    assert!(
        malformed(source, "add.i64", "soma.i64")
            .1
            .contains("instrução desconhecida 'soma.i64'")
    );
    assert!(
        malformed(source, "add.i64", "add")
            .1
            .contains("'add' leva 1 tipo(s)")
    );
    assert!(
        malformed(source, "add.i64", "add.real")
            .1
            .contains("tipo desconhecido 'real'")
    );
    assert!(
        malformed(source, "add.i64", "add.bool")
            .1
            .contains("esperava um tipo inteiro")
    );
    assert!(
        malformed(source, "add.i64", "add.i64 7")
            .1
            .contains("operando inesperado")
    );
    assert!(
        malformed(source, "load a#0", "load")
            .1
            .contains("falta o operando")
    );
    assert!(
        malformed(source, "load a#0", "load b#0")
            .1
            .contains("se chama 'a', não 'b'")
    );
    assert!(
        malformed(source, "load a#0", "load a#5")
            .1
            .contains("#5 não está na tabela")
    );
    assert!(
        malformed(source, "const.i64 1", "const.i64 +1")
            .1
            .contains("esperava um inteiro decimal")
    );
    assert!(
        malformed(source, "say.i64", "call sumida")
            .1
            .contains("a função 'sumida' não existe")
    );
    let (line, message) = malformed(source, "\n2   ", "\n7   ");
    assert!(
        message.contains("esperava a instrução 2, encontrei '7'"),
        "{message}"
    );
    assert!(line > 1);
}

#[test]
fn secoes_de_codigo_malformadas() {
    let source = "fun f()\n    say(1)\n\nfun main()\n    f()\n";
    assert!(
        malformed(source, "[código f 1:1]", "[código main 1:1]")
            .1
            .contains("esperava a seção [código f <linha:coluna>]")
    );
    assert!(
        malformed(source, "[código f 1:1]", "[código f]")
            .1
            .contains("posição inválida")
    );
    let content = rendered(source);
    let cut = content.find("\n[código main").unwrap();
    let e = bytecode::read(&content[..cut]).unwrap_err();
    assert!(
        e.message.contains("falta a seção [código main]"),
        "{}",
        e.message
    );
    let e = bytecode::read(&format!("{content}\n[código extra 1:1]\n")).unwrap_err();
    assert!(
        e.message.contains("seção de código a mais"),
        "{}",
        e.message
    );
    assert!(
        malformed(source, "1:5         ()", "1:5         (i64)")
            .1
            .contains(
                "a assinatura de 'f' diz (i64), mas os parâmetros da tabela de variáveis são ()"
            )
    );
}
// pink-nav: end
