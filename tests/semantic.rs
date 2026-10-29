//! Testes da análise semântica: nomes, tipos, instruções, fluxo e avaliação constante.

use pinker::checked::{self, CheckedProgram, IntType, Type};
use pinker::lexer::tokenize;
use pinker::parser::parse;
use pinker::semantic::check;

// pink-nav: start
// pink-resume: Auxiliares: verificam um programa inteiro ou o corpo de uma main e devolvem a posição do erro.
// pink-keywords: auxiliar, helper, checked, error_at, main
fn checked(source: &str) -> CheckedProgram {
    let tokens = tokenize(source).unwrap_or_else(|e| panic!("erro léxico em {source:?}: {e:?}"));
    let program = parse(&tokens).unwrap_or_else(|e| panic!("erro de sintaxe em {source:?}: {e:?}"));
    check(&program).unwrap_or_else(|e| panic!("a análise deveria aceitar {source:?}: {e:?}"))
}

/// A mensagem, a linha e a coluna do erro semântico.
fn error_at(source: &str) -> (String, usize, usize) {
    let tokens = tokenize(source).unwrap_or_else(|e| panic!("erro léxico em {source:?}: {e:?}"));
    let program = parse(&tokens).unwrap_or_else(|e| panic!("erro de sintaxe em {source:?}: {e:?}"));
    let e = check(&program).expect_err("a análise deveria rejeitar a entrada");
    (e.message, e.position.line, e.position.column)
}

/// Monta um programa com uma `main` cujo corpo são as linhas dadas.
fn main_with(lines: &[&str]) -> String {
    let mut source = String::from("fun main()\n");
    for line in lines {
        source.push_str("    ");
        source.push_str(line);
        source.push('\n');
    }
    source
}

fn accepts(lines: &[&str]) {
    checked(&main_with(lines));
}

/// A mensagem do erro numa `main` com as linhas dadas.
fn rejects(lines: &[&str]) -> String {
    error_at(&main_with(lines)).0
}
// pink-nav: end

// pink-nav: start
// pink-resume: Assinaturas e main: chamada antes da declaração, recursão, duplicatas, tipos desconhecidos, int como i64.
// pink-keywords: teste, assinatura, recursão, duplicata, tipo desconhecido, int, i64, main
#[test]
fn chamada_antes_da_declaracao_e_recursao() {
    checked(
        "fun main()\n    say(fat(5))\n\nfun fat(n: int) -> int\n    if n <= 1\n        give 1\n    give n * fat(n - 1)\n",
    );
}

#[test]
fn funcao_duplicada_e_erro() {
    let (message, line, column) =
        error_at("fun f()\n    give\n\nfun f()\n    give\n\nfun main()\n    f()\n");
    assert_eq!((line, column), (4, 5));
    assert!(message.contains("já foi declarada em 1:5"), "{message}");
}

#[test]
fn tipo_desconhecido_e_erro() {
    let (message, line, column) = error_at("fun f(x: float)\n    give\n\nfun main()\n    f(1)\n");
    assert_eq!((line, column), (1, 10));
    assert!(message.contains("tipo desconhecido 'float'"), "{message}");
}

#[test]
fn int_e_apelido_de_i64() {
    let program = checked("fun f(x: int) -> i64\n    give x\n\nfun main()\n    say(f(1))\n");
    let f = &program.functions[0];
    assert_eq!(f.variables[0].ty, Type::Int(IntType::I64));
    assert_eq!(f.result, Some(Type::Int(IntType::I64)));
}

#[test]
fn main_e_obrigatoria_e_sem_parametros_nem_resultado() {
    let (message, ..) = error_at("fun f()\n    give\n");
    assert!(message.contains("não tem uma função 'main'"), "{message}");
    let (message, line, column) = error_at("fun main(x: int)\n    say(x)\n");
    assert_eq!((line, column), (1, 10));
    assert!(message.contains("não recebe parâmetros"), "{message}");
    let (message, line, column) = error_at("fun main() -> int\n    give 0\n");
    assert_eq!((line, column), (1, 15));
    assert!(message.contains("não devolve resultado"), "{message}");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Nomes e escopos: não declarados, uso antes da declaração, sombreamento, duplicatas e predefinidos.
// pink-keywords: teste, nome, escopo, sombreamento, duplicata, predefinido, say, morph
#[test]
fn nome_nao_declarado_e_erro() {
    let (message, line, column) = error_at(&main_with(&["say(x)"]));
    assert_eq!((line, column), (2, 9));
    assert!(message.contains("'x' não foi declarado"), "{message}");
}

#[test]
fn valor_do_let_nao_ve_a_propria_variavel() {
    assert!(rejects(&["let x = x + 1"]).contains("'x' não foi declarado"));
}

#[test]
fn sombreamento_em_bloco_interno() {
    let program = checked(&main_with(&[
        "let x = 1",
        "if true",
        "    let x = true",
        "    say(x)",
        "say(x)",
    ]));
    let variables = &program.functions[0].variables;
    assert_eq!(variables.len(), 2);
    assert_eq!(variables[0].ty, Type::Int(IntType::I64));
    assert_eq!(variables[1].ty, Type::Bool);
}

#[test]
fn duplicata_no_mesmo_escopo_e_erro() {
    let message = rejects(&["let x = 1", "let x = 2"]);
    assert!(
        message.contains("já foi declarado neste escopo, em 2:9"),
        "{message}"
    );
}

#[test]
fn parametro_e_corpo_dividem_o_escopo() {
    let (message, ..) = error_at("fun f(a: int)\n    let a = 2\n\nfun main()\n    f(1)\n");
    assert!(
        message.contains("já foi declarado neste escopo"),
        "{message}"
    );
}

#[test]
fn variavel_some_ao_fim_do_bloco() {
    assert!(rejects(&["if true", "    let y = 1", "say(y)"]).contains("'y' não foi declarado"));
}

#[test]
fn funcao_nao_e_valor() {
    let message = rejects(&["let f = main"]);
    assert!(message.contains("'main' é uma função"), "{message}");
}

#[test]
fn predefinidas_nao_podem_ser_redeclaradas() {
    let (message, ..) = error_at("fun say()\n    give\n\nfun main()\n    give\n");
    assert!(
        message.contains("'say' é uma função predefinida"),
        "{message}"
    );
    assert!(rejects(&["let morph = 1"]).contains("predefinida"));
    let (message, ..) = error_at("fun f(say: int)\n    give\n\nfun main()\n    f(1)\n");
    assert!(message.contains("predefinida"), "{message}");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Tipos de expressões: inferência dos literais, faixas, operadores e ausência de promoção entre inteiros.
// pink-keywords: teste, literal, inferência, faixa, operador, promoção, condição, bool, text
#[test]
fn literal_recebe_o_tipo_do_contexto() {
    accepts(&[
        "let a: u8 = 255",
        "let b: i8 = -128",
        "let c: u64 = 18446744073709551615",
    ]);
    let message = rejects(&["let a: u8 = 256"]);
    assert!(
        message.contains("o valor 256 não cabe em u8 (faixa: 0 a 255)"),
        "{message}"
    );
    assert!(rejects(&["let a: u8 = -1"]).contains("o valor -1 não cabe em u8"));
}

#[test]
fn minimo_de_i64_e_aceito() {
    accepts(&[
        "let a = -9223372036854775808",
        "let b: i64 = -0x8000000000000000",
    ]);
    assert!(rejects(&["let a = 9223372036854775808"]).contains("não cabe em i64"));
}

#[test]
fn literal_recebe_o_tipo_do_outro_operando() {
    accepts(&[
        "let x: u8 = 1",
        "let y: u8 = x + 1",
        "let z: u8 = 1 + x",
        "let w = x == 3",
    ]);
}

#[test]
fn inteiros_de_tipos_diferentes_nao_se_misturam() {
    let message = rejects(&["let a: i32 = 1", "let b: i64 = 2", "say(a + b)"]);
    assert!(
        message.contains("mesmo tipo; encontrei i32 e i64"),
        "{message}"
    );
    let message = rejects(&["let a: i32 = 1", "let b: i64 = a"]);
    assert!(
        message.contains("esperava um valor do tipo i64, encontrei i32"),
        "{message}"
    );
}

#[test]
fn tabela_de_operadores() {
    accepts(&[
        "let a = 1 < 2",
        "let b = \"x\" == \"y\"",
        "let c = true != false",
        "let d = not a and b or c",
        "let e = -(3 * 4)",
    ]);
    assert!(rejects(&["let x = \"a\" < \"b\""]).contains("exige inteiros; encontrei text"));
    assert!(rejects(&["let x = true + 1"]).contains("mesmo tipo"));
    assert!(rejects(&["let x = not 1"]).contains("esperava um valor do tipo bool"));
    assert!(rejects(&["let x = 1 and true"]).contains("esperava um valor do tipo bool"));
    assert!(rejects(&["let x: u8 = 1", "let y = -x"]).contains("inteiro com sinal; encontrei u8"));
}

#[test]
fn condicoes_exigem_bool() {
    assert!(
        rejects(&["if 1", "    say(1)"]).contains("esperava um valor do tipo bool, encontrei i64")
    );
    assert!(rejects(&["while \"sim\"", "    break"]).contains("encontrei text"));
}
// pink-nav: end

// pink-nav: start
// pink-resume: Avaliação constante: overflow, divisão por zero, mínimo por -1 e respeito ao curto-circuito.
// pink-keywords: teste, constante, overflow, divisão por zero, curto-circuito, and, or
#[test]
fn overflow_constante_e_erro() {
    let message = rejects(&["let x: u8 = 200 + 100"]);
    assert!(
        message.contains("overflow: 200 + 100 = 300 não cabe em u8"),
        "{message}"
    );
    assert!(rejects(&["let x = 9223372036854775807 + 1"]).contains("overflow"));
    assert!(rejects(&["let x = -9223372036854775808 / -1"]).contains("overflow"));
    assert!(rejects(&["let x: i8 = -(-128)"]).contains("overflow"));
    accepts(&["let x = -9223372036854775808 % -1"]);
}

#[test]
fn divisao_por_zero_constante_e_erro() {
    assert_eq!(rejects(&["let x = 1", "let y = x / 0"]), "divisão por zero");
    assert_eq!(rejects(&["let y = 5 % (2 - 2)"]), "divisão por zero");
}

#[test]
fn curto_circuito_evita_falha_de_avaliacao() {
    accepts(&["let a = false and 1 / 0 == 1", "let b = true or 1 / 0 == 1"]);
    assert_eq!(
        rejects(&["let a = true and 1 / 0 == 1"]),
        "divisão por zero"
    );
}

#[test]
fn curto_circuito_ainda_verifica_tipos() {
    assert!(rejects(&["let a = false and 1 + true == 1"]).contains("mesmo tipo"));
    assert!(rejects(&["let a = false and y"]).contains("'y' não foi declarado"));
}
// pink-nav: end

// pink-nav: start
// pink-resume: Chamadas: quantidade e tipos dos argumentos, funções sem resultado, say e morph.
// pink-keywords: teste, chamada, argumento, say, morph, conversão, resultado
#[test]
fn argumentos_conferem_com_a_assinatura() {
    let source = "fun soma(a: int, b: int) -> int\n    give a + b\n\nfun main()\n";
    let (message, ..) = error_at(&format!("{source}    say(soma(1))\n"));
    assert!(
        message.contains("'soma' recebe 2 argumentos; recebeu 1"),
        "{message}"
    );
    let (message, ..) = error_at(&format!("{source}    say(soma(1, true))\n"));
    assert!(
        message.contains("esperava um valor do tipo i64, encontrei bool"),
        "{message}"
    );
}

#[test]
fn funcao_sem_resultado_nao_e_valor() {
    let (message, ..) = error_at("fun f()\n    give\n\nfun main()\n    let x = f()\n");
    assert!(message.contains("'f' não devolve resultado"), "{message}");
}

#[test]
fn resultado_pode_ser_descartado_numa_chamada_solta() {
    checked("fun f() -> int\n    give 1\n\nfun main()\n    f()\n");
}

#[test]
fn say_recebe_um_valor_de_qualquer_tipo() {
    accepts(&[
        "let a: u16 = 1",
        "say(a)",
        "say(true)",
        "say(\"olá\")",
        "say(-1)",
    ]);
    assert!(rejects(&["say(1, 2)"]).contains("exatamente 1 argumento; recebeu 2"));
    assert!(rejects(&["say(say(1))"]).contains("'say' não devolve resultado"));
}

#[test]
fn morph_converte_entre_inteiros_com_destino_do_contexto() {
    accepts(&[
        "let a: i32 = 7",
        "let b: i64 = morph(a)",
        "let c: u8 = morph(b)",
    ]);
    assert!(rejects(&["let a: i32 = 7", "let b = morph(a)"]).contains("tipo de destino"));
    assert!(rejects(&["let b: i64 = morph(true)"]).contains("só entre inteiros; encontrei bool"));
    assert!(rejects(&["let b: u8 = morph(300)"]).contains("300 não cabe em u8"));
}
// pink-nav: end

// pink-nav: start
// pink-resume: Instruções e fluxo: mutabilidade, give compatível, break fora de laço e caminhos sem give.
// pink-keywords: teste, mutabilidade, atribuição, give, break, continue, fluxo, retorno
#[test]
fn atribuicao_exige_variavel_mutavel_do_mesmo_tipo() {
    accepts(&["let mut x = 1", "x = x + 1"]);
    assert!(rejects(&["let x = 1", "x = 2"]).contains("não é mutável; declare-a com 'let mut'"));
    assert!(rejects(&["let mut x = 1", "x = true"]).contains("esperava um valor do tipo i64"));
    let (message, ..) = error_at("fun f(a: int)\n    a = 2\n\nfun main()\n    f(1)\n");
    assert!(message.contains("o parâmetro 'a' é imutável"), "{message}");
}

#[test]
fn give_confere_com_o_resultado() {
    assert!(rejects(&["give 1"]).contains("não devolve resultado; use 'give' sem valor"));
    let (message, line, column) = error_at("fun f() -> int\n    give\n\nfun main()\n    f()\n");
    assert_eq!((line, column), (2, 5));
    assert!(
        message.contains("'give' sem valor numa função que devolve i64"),
        "{message}"
    );
}

#[test]
fn break_e_continue_so_dentro_de_lacos() {
    let (message, line, column) = error_at(&main_with(&["break"]));
    assert_eq!((line, column), (2, 5));
    assert_eq!(message, "'break' fora de um laço");
    assert_eq!(
        rejects(&["if true", "    continue"]),
        "'continue' fora de um laço"
    );
    accepts(&[
        "while true",
        "    if false",
        "        break",
        "    continue",
    ]);
}

#[test]
fn funcao_com_resultado_precisa_devolver_em_todos_os_caminhos() {
    let missing = |body: &str| {
        error_at(&format!(
            "fun f(x: bool) -> int\n{body}\nfun main()\n    say(f(true))\n"
        ))
    };
    let (message, line, column) = missing("    if x\n        give 1\n");
    assert_eq!((line, column), (1, 5));
    assert!(
        message.contains("pode chegar ao fim sem devolver um valor do tipo i64"),
        "{message}"
    );
    assert!(
        missing("    while x\n        give 1\n")
            .0
            .contains("pode chegar ao fim")
    );
    assert!(
        missing("    loop\n        break\n")
            .0
            .contains("pode chegar ao fim")
    );
}

#[test]
fn caminhos_que_sempre_devolvem() {
    let ok = |body: &str| {
        checked(&format!(
            "fun f(x: bool) -> int\n{body}\nfun main()\n    say(f(true))\n"
        ))
    };
    ok("    if x\n        give 1\n    else\n        give 2\n");
    ok("    if x\n        give 1\n    else if not x\n        give 2\n    else\n        give 3\n");
    ok("    loop\n        if x\n            give 1\n");
    ok("    loop\n        while x\n            break\n        give 1\n");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Saída de --emit=semantic: a tabela de símbolos, os tipos de cada nó e as variáveis numeradas, mostrando o sombreamento.
// pink-keywords: teste, emit, semantic, render, tipos, vínculos, tabela de símbolos
#[test]
fn render_mostra_tipos_e_vinculos() {
    let source = "fun dobro(n: u8) -> u8\n    give n * 2\n\nfun main()\n    let n = dobro(3)\n    if true\n        let n = -1\n        say(n)\n";
    let expected = "\
# pinker semantic 2 · fonte: teste.pk
[funções]
#NOME  DECLARAÇÃO  ASSINATURA
dobro  1:5         (u8) -> u8
main   4:5         ()

[variáveis dobro]
#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE
0   n     u8    1:11        parâmetro

[variáveis main]
#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE
0   n     u8    5:9         let
1   n     i64   7:13        let

[árvore]
#POSIÇÃO NÓ
1:1      Função dobro
1:11       Parâmetro n#0
2:5        Bloco
2:5          Give
2:12           Binária *: u8
2:10             Variável n#0: u8
2:14             Inteiro 2: u8
4:1      Função main
5:5        Bloco
5:5          Let n#0
5:13           Chamada dobro: u8
5:19             Inteiro 3: u8
6:5          If
6:8            Lógico true: bool
7:9            Bloco
7:9              Let n#1
7:17               Inteiro -1: i64
8:9              Chamada say (predefinida)
8:13               Variável n#1: i64
";
    assert_eq!(checked::render(&checked(source), "teste.pk"), expected);
}
// pink-nav: end

// pink-nav: start
// pink-resume: Código que comprovadamente não executa: ramos de condição constante não geram falha de avaliação, mas ainda verificam tipos.
// pink-keywords: teste, código morto, if false, while false, avaliação constante, tipos
#[test]
fn ramo_que_nao_executa_nao_gera_falha_de_avaliacao() {
    accepts(&["if false", "    let value = 1 / 0"]);
    accepts(&["if true", "    say(1)", "else", "    let value = 1 / 0"]);
    accepts(&["if true", "    say(1)", "else if 1 / 0 == 1", "    say(2)"]);
    accepts(&["while false", "    let value: u8 = 200 + 100"]);
}

#[test]
fn ramo_que_nao_executa_ainda_verifica_tipos_e_literais() {
    assert!(
        rejects(&["if false", "    let value: int = \"texto\""])
            .contains("esperava um valor do tipo i64")
    );
    assert!(rejects(&["if false", "    let value: u8 = 256"]).contains("não cabe em u8"));
    assert!(rejects(&["if false", "    say(y)"]).contains("'y' não foi declarado"));
}

#[test]
fn condicao_nao_constante_mantem_a_avaliacao() {
    assert_eq!(
        rejects(&["let c = true", "if c", "    let value = 1 / 0"]),
        "divisão por zero"
    );
    assert_eq!(
        rejects(&["if true", "    let value = 1 / 0"]),
        "divisão por zero"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Fronteiras: mínimos e máximos dos inteiros, conversões entre sinais, break de laço aninhado e loop sem fim.
// pink-keywords: teste, fronteira, mínimo, máximo, conversão, sinal, laço aninhado, break, loop
#[test]
fn minimos_e_maximos_de_cada_tipo() {
    accepts(&[
        "let a: i8 = -128",
        "let b: i16 = -32768",
        "let c: i32 = -2147483648",
        "let d: i64 = -9223372036854775808",
        "let e: u64 = 18446744073709551615",
        "let f: i32 = 2147483647",
    ]);
    assert!(rejects(&["let a: i16 = -32769"]).contains("não cabe em i16"));
    assert!(rejects(&["let a: u64 = 18446744073709551616"]).contains("não cabe em u64"));
}

#[test]
fn morph_entre_tipos_com_e_sem_sinal_verifica_o_valor() {
    accepts(&[
        "let a: u64 = 5",
        "let b: i64 = morph(a)",
        "let c: i8 = -1",
        "let d: u8 = morph(c)",
    ]);
    assert!(
        rejects(&["let d: u8 = morph(-1)"]).contains("conversão fora da faixa: -1 não cabe em u8")
    );
    let message = rejects(&["let d: i64 = morph(18446744073709551615)"]);
    assert!(message.contains("não cabe em i64"), "{message}");
}

#[test]
fn break_de_laco_interno_nao_encerra_o_externo() {
    let ok = |body: &str| {
        checked(&format!(
            "fun f() -> int\n{body}\nfun main()\n    say(f())\n"
        ))
    };
    ok("    loop\n        continue\n");
    ok("    loop\n        loop\n            break\n");
    ok("    loop\n        while true\n            break\n");
    let (message, ..) = error_at(
        "fun f() -> int\n    loop\n        loop\n            break\n        break\n\nfun main()\n    say(f())\n",
    );
    assert!(message.contains("pode chegar ao fim"), "{message}");
}
// pink-nav: end
