//! Testes dos arquivos do pipeline (.t, .a, .s): ida e volta, erros de formato e o
//! verificador da árvore lida de um .s.

use pinker::artifact::{self, Format};
use pinker::checked::{self, CheckedProgram};
use pinker::interpreter::run;
use pinker::lexer::tokenize;
use pinker::parser::parse;
use pinker::semantic::check;
use pinker::{ast, token, verifier};

// pink-nav: start
// pink-resume: Programa de teste com todas as construções da primeira versão e auxiliares de cada etapa.
// pink-keywords: auxiliar, corpus, programa completo, etapas
/// Usa todas as construções da primeira versão, inclusive textos com escapes e Unicode.
const COMPLETE: &str = r#"# Programa com todas as construções da primeira versão.
fun fatorial(n: u64) -> u64
    if n <= 1
        give 1
    give n * fatorial(n - 1)

fun classifica(x: int) -> text
    if x > 0
        give "positivo"
    else if x < 0
        give "negativo"
    else
        give "zero: \"nada\"\n\tfim"

fun conta(limite: i32,)
    let mut i: i32 = 0
    while true
        i = i + 1
        if i % 2 == 0
            continue
        if i > limite
            break
        say(i)
    loop
        break
    give

fun main()
    let minimo = -9223372036854775808
    let mascara: u8 = 0XfF
    let bits = 0b10_10
    let largo: i64 = morph(mascara)
    say(fatorial(10))
    say(classifica(-3))
    say(classifica(0))
    say("olá, Pinker")
    say(not (largo == 255) or false and true)
    conta(5)
    let x = 1
    if true
        let x = "sombra"
        say(x)
    say(minimo + x + bits)
"#;

fn checked_program(source: &str) -> CheckedProgram {
    check(&parse(&tokenize(source).unwrap()).unwrap()).unwrap()
}

fn output(program: &CheckedProgram) -> String {
    let mut output = Vec::new();
    run(program, &mut output).unwrap();
    String::from_utf8(output).unwrap()
}

/// O formato e a fonte do cabeçalho, conferindo que é o esperado.
fn assert_header(content: &str, format: Format) {
    assert_eq!(
        artifact::detect(content).unwrap(),
        Some((format, "c.pk".to_string()))
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Ida e volta: gravar e ler cada formato devolve exatamente a mesma estrutura e a mesma execução.
// pink-keywords: teste, ida e volta, round trip, tokens, ast, semantic, equivalência
#[test]
fn tokens_ida_e_volta() {
    let tokens = tokenize(COMPLETE).unwrap();
    let content = token::render(&tokens, "c.pk");
    assert_header(&content, Format::Tokens);
    assert_eq!(token::read(&content).unwrap(), tokens);
}

#[test]
fn ast_ida_e_volta() {
    let program = parse(&tokenize(COMPLETE).unwrap()).unwrap();
    let content = ast::render(&program, "c.pk");
    assert_header(&content, Format::Ast);
    assert_eq!(ast::read(&content).unwrap(), program);
}

#[test]
fn arvore_verificada_ida_e_volta() {
    let program = checked_program(COMPLETE);
    let content = checked::render(&program, "c.pk");
    assert_header(&content, Format::Semantic);
    let read = checked::read(&content).unwrap();
    assert_eq!(read, program);
    verifier::verify(&read).unwrap();
    assert_eq!(output(&read), output(&program));
}

#[test]
fn programa_completo_executa() {
    assert_eq!(
        output(&checked_program(COMPLETE)),
        "3628800\nnegativo\nzero: \"nada\"\n\tfim\nolá, Pinker\nfalse\n1\n3\n5\nsombra\n-9223372036854775797\n"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Erros de formato: cabeçalho, tokens inválidos, árvore malformada, seções do .s e a árvore que discorda da tabela de símbolos.
// pink-keywords: teste, erro de formato, cabeçalho, versão, token, indentação, variável, função, tabela de símbolos, seção
fn format_error<T: std::fmt::Debug>(result: Result<T, artifact::FormatError>) -> (usize, String) {
    let e = result.expect_err("a leitura deveria falhar");
    (e.line, e.message)
}

#[test]
fn cabecalho_invalido() {
    assert_eq!(artifact::detect("fun main()\n").unwrap(), None);
    let (_, message) = format_error(artifact::detect("# pinker tokens 2 · fonte: a.pk\n"));
    assert!(message.contains("versão '2'"), "{message}");
    // O .s ganhou a tabela de símbolos na versão 2; a versão 1 não é mais lida.
    let (_, message) = format_error(artifact::detect("# pinker semantic 1 · fonte: a.pk\n"));
    assert!(message.contains("lê a versão 2"), "{message}");
    let (_, message) = format_error(artifact::detect("# pinker assembly 1 · fonte: a.pk\n"));
    assert!(
        message.contains("formato desconhecido 'assembly'"),
        "{message}"
    );
    let (line, message) = format_error(artifact::detect("# pinker tokens 1\n"));
    assert_eq!(line, 1);
    assert!(message.contains("cabeçalho inválido"), "{message}");
}

#[test]
fn tokens_invalidos() {
    let header = "# pinker tokens 1 · fonte: a.pk\n";
    let read = |body: &str| format_error(token::read(&format!("{header}{body}")));
    assert_eq!(
        read("1:1 PALAVRA soma\n1:5 FIM\n"),
        (2, "'soma' não é uma palavra reservada".into())
    );
    assert_eq!(
        read("1:1 COISA x\n").1,
        "tipo de token desconhecido 'COISA'"
    );
    assert_eq!(
        read("1:1 IDENTIFICADOR x\n"),
        (2, "falta o token FIM no fim do arquivo".into())
    );
    assert_eq!(read("1:1 FIM\n1:2 FIM\n").1, "token depois de FIM");
    assert!(read("x:1 FIM\n").1.contains("posição inválida"));
    assert!(read("1:1 INTEIRO 0xZ\n").1.contains("literal inválido"));
    let (_, message) = read("1:1 INTEIRO 1__0\n");
    assert!(message.contains("separador '_'"), "{message}");
    assert!(read("1:1 TEXTO 12\n").1.contains("não é um TEXTO"));
    assert!(read("1:1 QUEBRA x\n").1.contains("não tem lexema"));
}

#[test]
fn comentarios_e_linhas_em_branco_sao_ignorados() {
    let content = "# pinker tokens 1 · fonte: a.pk\n# comentário\n\n1:1      IDENTIFICADOR  x\n1:2      FIM\n";
    assert_eq!(token::read(content).unwrap().len(), 2);
}

#[test]
fn arvore_malformada() {
    let header = "# pinker ast 1 · fonte: a.pk\n";
    let read = |body: &str| format_error(ast::read(&format!("{header}{body}")));
    let (line, message) = read("1:1      Função f@1:5\n2:5         Bloco\n");
    assert_eq!(line, 3);
    assert!(message.contains("indentação inválida"), "{message}");
    assert!(
        read("1:1      Função f@1:5\n")
            .1
            .contains("falta o bloco da função")
    );
    assert!(
        read("1:1      Função f@1:5\n2:5        Bloco\n")
            .1
            .contains("bloco vazio")
    );
    assert!(
        read("1:1      Função f@1:5\n2:5        Bloco\n2:5          Pular\n")
            .1
            .contains("instrução desconhecida 'Pular'")
    );
    let (line, message) = read(
        "1:1      Função f@1:5\n2:5        Bloco\n2:5          Break\n2:5            Nome x\n",
    );
    assert_eq!(line, 5);
    assert!(message.contains("nó inesperado"), "{message}");
    assert!(
        read("1:1      Função f@1:5\n2:5        Bloco\n2:5          Let x@2:9\n")
            .1
            .contains("falta uma expressão")
    );
}

/// Um .s com a tabela de `main` e a árvore dada para ela.
fn semantic(variables: &str, tree: &str) -> String {
    format!(
        "# pinker semantic 2 · fonte: a.pk\n[funções]\nmain  1:5  ()\n\n[variáveis main]\n{variables}\n[árvore]\n1:1      Função main\n2:5        Bloco\n{tree}"
    )
}

#[test]
fn arvore_precisa_concordar_com_a_tabela() {
    let read =
        |variables: &str, tree: &str| format_error(checked::read(&semantic(variables, tree)));
    let let_x = "2:5          Let x#0\n2:13           Inteiro 1: i64\n";
    assert!(checked::read(&semantic("0  x  i64  2:9  let\n", let_x)).is_ok());

    let (line, message) = read("0  x  i64  2:9  let\n1  y  i64  3:9  let\n", let_x);
    assert_eq!(line, 7);
    assert!(
        message.contains("a variável #1 de 'main' está na tabela, mas a árvore não a declara"),
        "{message}"
    );
    let (line, message) = read("", let_x);
    assert_eq!(line, 10);
    assert!(
        message.contains("#0 não está na tabela de 'main'"),
        "{message}"
    );
    let (_, message) = read(
        "0  x  i64  2:9  let\n",
        &format!(
            "{let_x}3:5          Chamada say (predefinida)\n3:9            Variável y#0: i64\n"
        ),
    );
    assert!(message.contains("se chama 'x', não 'y'"), "{message}");
    let (_, message) = read("0  x  i64  2:9  parâmetro\n", let_x);
    assert!(message.contains("é um parâmetro na tabela"), "{message}");
    let (_, message) = read(
        "0  x  i64  2:9  let\n",
        &format!("{let_x}{}", let_x.replace("2:", "3:")),
    );
    assert!(message.contains("declarada mais de uma vez"), "{message}");
    let (_, message) = read("0  x  i64  2:9  let\n", "2:5          Chamada sumida\n");
    assert!(
        message.contains("a função 'sumida' não existe"),
        "{message}"
    );
}

#[test]
fn tabela_de_simbolos_malformada() {
    let read = |variables: &str| {
        let tree = "2:5          Let x#0\n2:13           Inteiro 1: i64\n";
        format_error(checked::read(&semantic(variables, tree)))
    };
    assert_eq!(
        read("1  x  i64  2:9  let\n"),
        (6, "esperava a variável #0, encontrei '1'".into())
    );
    assert!(
        read("0  x  i64  2:9  let mutável\n")
            .1
            .contains("espécie desconhecida 'let mutável'")
    );
    assert!(
        read("0  x  real  2:9  let\n")
            .1
            .contains("tipo desconhecido 'real'")
    );
    assert!(read("0  x  i64  2-9  let\n").1.contains("posição inválida"));

    let file = |body: &str| {
        format_error(checked::read(&format!(
            "# pinker semantic 2 · fonte: a.pk\n{body}"
        )))
    };
    assert_eq!(
        file("1:1      Função main\n"),
        (2, "esperava a seção [funções]".into())
    );
    assert!(
        file("[funções]\nmain  1:5  ()\n")
            .1
            .contains("falta a seção [árvore]")
    );
    assert!(
        file("[funções]\nmain  1:5  ()\n[árvore]\n")
            .1
            .contains("falta a seção [variáveis main]")
    );
    assert!(
        file("[funções]\nmain  1:5  ()\n[variáveis f]\n[árvore]\n")
            .1
            .contains("esperava a seção [variáveis main]")
    );
    assert!(
        file("[funções]\n[árvore]\n[variáveis main]\n")
            .1
            .contains("desconhecida ou fora de ordem")
    );
    assert!(
        file("[funções]\nmain  1:5  i64\n[variáveis main]\n[árvore]\n")
            .1
            .contains("esperava 'nome  linha:coluna")
    );
    let (_, message) = file(
        "[funções]\nf  1:5  ()\n[variáveis f]\n[árvore]\n1:1      Função f\n2:5        Bloco\n2:5          Give\n",
    );
    assert!(message.contains("não tem uma função 'main'"), "{message}");
}

#[test]
fn assinatura_da_tabela_e_ordem_das_funcoes() {
    let source = "fun f(a: u8) -> u8\n    give a\n\nfun main()\n    say(f(1))\n";
    let content = checked::render(&checked_program(source), "c.pk");
    let read = |from: &str, to: &str| {
        assert!(content.contains(from), "{content}");
        format_error(checked::read(&content.replacen(from, to, 1))).1
    };
    let message = read("(u8) -> u8", "(u8, u8) -> u8");
    assert!(
        message.contains("a assinatura de 'f' diz (u8, u8), mas os parâmetros na árvore são (u8)"),
        "{message}"
    );
    let message = read("1:1      Função f\n", "1:1      Função g\n");
    assert!(message.contains("esperava a função 'f'"), "{message}");
    let message = read("f      1:5", "main   1:5");
    assert!(message.contains("função 'main' repetida"), "{message}");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Verificador: um .s editado à mão com tipos, escopo, fluxo ou mutabilidade incoerentes é recusado antes da execução.
// pink-keywords: teste, verificador, .s editado, tipo, escopo, fluxo, mutabilidade, break
/// Grava o programa como .s, aplica a edição e devolve a mensagem do verificador.
fn tampered(source: &str, from: &str, to: &str) -> String {
    let content = checked::render(&checked_program(source), "c.pk");
    assert!(content.contains(from), "{content}");
    let program = checked::read(&content.replacen(from, to, 1)).unwrap();
    verifier::verify(&program)
        .expect_err("o verificador deveria recusar")
        .message
}

#[test]
fn verificador_recusa_tipos_incoerentes() {
    let source = "fun main()\n    let x: u8 = 1\n    say(x)\n";
    assert!(
        tampered(source, "Inteiro 1: u8", "Inteiro 1: bool").contains("o tipo bool não é coerente")
    );
    assert!(tampered(source, "Inteiro 1: u8", "Inteiro 300: u8").contains("não é coerente"));
}

#[test]
fn verificador_recusa_variavel_fora_do_escopo() {
    let source = "fun main()\n    let a = 1\n    say(a)\n    let b = 2\n";
    let message = tampered(source, "Variável a#0: i64", "Variável b#1: i64");
    assert!(message.contains("fora do seu escopo"), "{message}");
}

#[test]
fn verificador_recusa_break_fora_de_laco() {
    let source = "fun main()\n    say(1)\n";
    let message = tampered(
        source,
        "2:5          Chamada say (predefinida)\n2:9            Inteiro 1: i64\n",
        "2:5          Break\n",
    );
    assert!(message.contains("fora de um laço"), "{message}");
}

#[test]
fn verificador_recusa_funcao_que_chega_ao_fim_sem_resultado() {
    let source = "fun f() -> int\n    give 1\n\nfun main()\n    say(f())\n";
    let message = tampered(
        source,
        "2:5          Give",
        "2:5          Chamada say (predefinida)",
    );
    assert!(message.contains("pode chegar ao fim"), "{message}");
}

#[test]
fn verificador_recusa_atribuicao_a_imutavel() {
    let source = "fun main()\n    let mut x = 1\n    x = 2\n";
    // A mutabilidade vem da tabela de símbolos: tirar o `mut` dela muda o programa lido.
    let message = tampered(source, "let mut\n", "let\n");
    assert!(message.contains("imutável"), "{message}");
}

#[test]
fn verificador_usa_o_tipo_da_tabela() {
    let source = "fun main()\n    let x: u8 = 1\n    say(x)\n";
    let message = tampered(source, "x     u8    2:9", "x     bool  2:9");
    assert!(
        message.contains("esperava bool, a expressão é u8"),
        "{message}"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Artefatos gravados pela versão d8b2948 (antes da concatenação) continuam aceitos, executam igual e são gravados com os mesmos bytes.
// pink-keywords: teste, compatibilidade, versão, artefato antigo, .s, .b, concatenação
/// O programa e os arquivos `.s` e `.b` que o `pinker` do commit d8b2948 gravou para ele.
const OLD_SOURCE: &str = r#"fun junta(a: text, n: u8) -> text
    if n > 1
        give a
    give "um"

fun main()
    let t = junta("dois", 2)
    say(t)
    say(t == "dois")
"#;
const OLD_SEMANTIC: &str = r#"# pinker semantic 2 · fonte: antigo.pk
[funções]
#NOME  DECLARAÇÃO  ASSINATURA
junta  1:5         (text, u8) -> text
main   6:5         ()

[variáveis junta]
#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE
0   a     text  1:11        parâmetro
1   n     u8    1:20        parâmetro

[variáveis main]
#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE
0   t     text  7:9         let

[árvore]
#POSIÇÃO NÓ
1:1      Função junta
1:11       Parâmetro a#0
1:20       Parâmetro n#1
2:5        Bloco
2:5          If
2:10           Binária >: bool
2:8              Variável n#1: u8
2:12             Inteiro 1: u8
3:9            Bloco
3:9              Give
3:14               Variável a#0: text
4:5          Give
4:10           Texto "um": text
6:1      Função main
7:5        Bloco
7:5          Let t#0
7:13           Chamada junta: text
7:19             Texto "dois": text
7:27             Inteiro 2: u8
8:5          Chamada say (predefinida)
8:9            Variável t#0: text
9:5          Chamada say (predefinida)
9:11           Binária ==: bool
9:9              Variável t#0: text
9:14             Texto "dois": text
"#;
const OLD_BYTECODE: &str = r#"# pinker bytecode 1 · fonte: antigo.pk
[funções]
#NOME  DECLARAÇÃO  ASSINATURA
junta  1:5         (text, u8) -> text
main   6:5         ()

[variáveis junta]
#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE
0   a     text  1:11        parâmetro
1   n     u8    1:20        parâmetro

[variáveis main]
#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE
0   t     text  7:9         let

[código junta 1:1]
#N  POSIÇÃO  INSTRUÇÃO
0   2:8      load n#1
1   2:12     const.u8 1
2   2:10     gt.u8
3   2:5      jump_false 6
4   3:14     load a#0
5   3:9      give
6   4:10     const.text "um"
7   4:5      give

[código main 6:1]
#N  POSIÇÃO  INSTRUÇÃO
0   7:19     const.text "dois"
1   7:27     const.u8 2
2   7:13     call junta
3   7:5      store t#0
4   8:9      load t#0
5   8:5      say.text
6   9:9      load t#0
7   9:14     const.text "dois"
8   9:11     eq.text
9   9:5      say.bool
10  7:5      clear t#0
11  6:1      give
"#;

#[test]
fn artefatos_anteriores_a_concatenacao_continuam_aceitos() {
    use pinker::{bytecode, lower::lower, vm};
    let program = checked_program(OLD_SOURCE);
    assert_eq!(checked::render(&program, "antigo.pk"), OLD_SEMANTIC);
    assert_eq!(
        bytecode::render(&lower(&program), "antigo.pk"),
        OLD_BYTECODE
    );

    let read = checked::read(OLD_SEMANTIC).unwrap();
    verifier::verify(&read).unwrap();
    assert_eq!(output(&read), "dois\ntrue\n");
    let compiled = bytecode::read(OLD_BYTECODE).unwrap();
    bytecode::verify(&compiled).unwrap();
    let mut out = Vec::new();
    vm::run(&compiled, &mut out).unwrap();
    assert_eq!(out, b"dois\ntrue\n");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Concatenação no .s: ida e volta, e o verificador recusa um + de textos com tipo ou operando incoerente.
// pink-keywords: teste, concatenação, .s, ida e volta, verificador, adulteração, texto
const CONCAT: &str = "fun junta(a: text) -> text\n    give a + \"!\"\n\nfun main()\n    let a = \"x\"\n    say(junta(a + \"y\") + \"\")\n";

#[test]
fn arvore_com_concatenacao_ida_e_volta() {
    let program = checked_program(CONCAT);
    let content = checked::render(&program, "c.pk");
    assert!(content.contains("Binária +: text"), "{content}");
    let read = checked::read(&content).unwrap();
    assert_eq!(read, program);
    verifier::verify(&read).unwrap();
    assert_eq!(output(&read), "xy!\n");
}

#[test]
fn verificador_recusa_concatenacao_incoerente() {
    let message = tampered(CONCAT, "Binária +: text", "Binária +: i64");
    assert!(message.contains("o tipo i64 não é coerente"), "{message}");
    let message = tampered(CONCAT, "Texto \"!\": text", "Inteiro 1: i64");
    assert!(message.contains("o tipo text não é coerente"), "{message}");
}
// pink-nav: end
