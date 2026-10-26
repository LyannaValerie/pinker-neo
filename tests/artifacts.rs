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
    let bits = 0b1010
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
// pink-resume: Erros de formato: cabeçalho, tokens inválidos, árvore malformada e numeração de variáveis do .s.
// pink-keywords: teste, erro de formato, cabeçalho, versão, token, indentação, variável, função
fn format_error<T: std::fmt::Debug>(result: Result<T, artifact::FormatError>) -> (usize, String) {
    let e = result.expect_err("a leitura deveria falhar");
    (e.line, e.message)
}

#[test]
fn cabecalho_invalido() {
    assert_eq!(artifact::detect("fun main()\n").unwrap(), None);
    let (_, message) = format_error(artifact::detect("# pinker tokens 2 · fonte: a.pk\n"));
    assert!(message.contains("versão '2'"), "{message}");
    let (_, message) = format_error(artifact::detect("# pinker bytecode 1 · fonte: a.pk\n"));
    assert!(
        message.contains("formato desconhecido 'bytecode'"),
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

#[test]
fn variaveis_e_funcoes_do_s_precisam_existir() {
    let header = "# pinker semantic 1 · fonte: a.pk\n";
    let read = |body: &str| format_error(checked::read(&format!("{header}{body}")));
    let main = "1:1      Função main\n2:5        Bloco\n";
    let (_, message) = read(&format!(
        "{main}2:5          Let x#1@2:9: i64\n2:13           Inteiro 1: i64\n"
    ));
    assert!(
        message.contains("a variável #0 de 'main' não é declarada"),
        "{message}"
    );
    let (_, message) = read(&format!(
        "{main}2:5          Let x#0@2:9: i64\n2:13           Inteiro 1: i64\n3:5          Chamada say (predefinida)\n3:9            Variável y#0: i64\n"
    ));
    assert!(message.contains("se chama 'x', não 'y'"), "{message}");
    let (_, message) = read(&format!("{main}2:5          Chamada sumida\n"));
    assert!(
        message.contains("a função 'sumida' não existe"),
        "{message}"
    );
    let (_, message) = read("1:1      Função f\n2:5        Bloco\n2:5          Give\n");
    assert!(message.contains("não tem uma função 'main'"), "{message}");
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
    let message = tampered(source, "Let mut x#0", "Let x#0");
    assert!(message.contains("imutável"), "{message}");
}
// pink-nav: end
