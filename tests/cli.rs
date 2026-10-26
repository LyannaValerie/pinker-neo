//! Testes da linha de comando `pinker`.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

// pink-nav: start
// pink-resume: Auxiliares: o executável pinker e um arquivo .pk de teste num diretório temporário.
// pink-keywords: auxiliar, helper, executável, arquivo temporário
fn pinker() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pinker"))
}

/// Cria `<nome>/ola.pk` com o conteúdo dado e devolve o caminho.
fn source_file(name: &str, content: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ola.pk");
    fs::write(&path, content).unwrap();
    path
}
// pink-nav: end

// pink-nav: start
// pink-resume: --emit=tokens grava o arquivo padrão ou escreve na saída padrão; erros e opções inválidas.
// pink-keywords: teste, cli, emit, tokens, -o, código de saída, diagnóstico
#[test]
fn emit_tokens_grava_ao_lado_da_entrada() {
    let input = source_file("emit_padrao", "say(\"olá\")\n");
    let status = pinker().arg(&input).arg("--emit=tokens").status().unwrap();
    assert!(status.success());
    let content = fs::read_to_string(input.with_extension("tokens")).unwrap();
    assert!(
        content.starts_with("POSIÇÃO  TIPO           LEXEMA\n"),
        "{content}"
    );
    assert!(
        content.contains("1:1      IDENTIFICADOR  say\n"),
        "{content}"
    );
    assert!(
        content.contains("1:5      TEXTO          \"olá\"\n"),
        "{content}"
    );
}

#[test]
fn emit_tokens_na_saida_padrao() {
    let input = source_file("emit_stdout", "let x = 0xFF\n");
    let output = pinker()
        .arg(&input)
        .args(["--emit=tokens", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("1:9      INTEIRO        0xFF  (base 16)\n"),
        "{stdout}"
    );
}

#[test]
fn erro_lexico_mostra_arquivo_linha_e_coluna() {
    let input = source_file("erro_lexico", "say(\"abc\n");
    let output = pinker().arg(&input).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ola.pk:1:5: erro: texto não fechado"),
        "{stderr}"
    );
}

#[test]
fn erro_lexico_nao_grava_tokens() {
    let input = source_file("erro_sem_saida", "say(\"abc\n");
    let output = pinker().arg(&input).arg("--emit=tokens").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!input.with_extension("tokens").exists());
}

#[test]
fn etapa_desconhecida_e_erro_de_uso() {
    let input = source_file("etapa_desconhecida", "x\n");
    let output = pinker().arg(&input).arg("--emit=ir").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("etapa desconhecida 'ir'"), "{stderr}");
}
// pink-nav: end

// pink-nav: start
// pink-resume: --emit=ast grava a árvore; erro de sintaxe falha com posição; --emit=tokens não depende do parser.
// pink-keywords: teste, cli, emit, ast, parser, erro de sintaxe
#[test]
fn emit_ast_grava_a_arvore() {
    let input = source_file("emit_ast", "fun main()\n    say(1)\n");
    let status = pinker().arg(&input).arg("--emit=ast").status().unwrap();
    assert!(status.success());
    let content = fs::read_to_string(input.with_extension("ast")).unwrap();
    assert!(content.contains("1:1      Função main\n"), "{content}");
    assert!(content.contains("2:5          Chamada say\n"), "{content}");
}

#[test]
fn erro_de_sintaxe_mostra_arquivo_linha_e_coluna() {
    let input = source_file("erro_sintaxe", "fun main()\n    a + b\n");
    let output = pinker().arg(&input).arg("--emit=ast").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ola.pk:2:7: erro: esperava '=' ou '('"),
        "{stderr}"
    );
    assert!(!input.with_extension("ast").exists());
}

#[test]
fn emit_tokens_funciona_mesmo_com_erro_de_sintaxe() {
    let input = source_file("tokens_sem_parser", "a + b\n");
    let output = pinker()
        .arg(&input)
        .args(["--emit=tokens", "-o", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
}
// pink-nav: end

// pink-nav: start
// pink-resume: A CLI recusa gravar sobre o próprio arquivo de entrada, por caminho direto, link ou extensão padrão.
// pink-keywords: teste, cli, proteção, entrada, saída, symlink, hard link, -o
fn assert_refuses_and_preserves(input: &PathBuf, args: &[&str]) {
    let before = fs::read(input).unwrap();
    let output = pinker().arg(input).args(args).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("é o próprio arquivo de entrada"),
        "{stderr}"
    );
    assert_eq!(fs::read(input).unwrap(), before);
}

#[test]
fn saida_igual_a_entrada_e_recusada() {
    let input = source_file("saida_igual_entrada", "fun main()\n    say(1)\n");
    let path = input.to_str().unwrap().to_string();
    assert_refuses_and_preserves(&input, &["--emit=ast", "-o", &path]);
}

#[test]
fn saida_por_link_para_a_entrada_e_recusada() {
    let input = source_file("saida_por_link", "fun main()\n    say(1)\n");
    let dir = input.parent().unwrap();
    let symlink = dir.join("simbolico.ast");
    let hardlink = dir.join("fisico.ast");
    let _ = fs::remove_file(&symlink);
    let _ = fs::remove_file(&hardlink);
    std::os::unix::fs::symlink(&input, &symlink).unwrap();
    fs::hard_link(&input, &hardlink).unwrap();
    for alias in [&symlink, &hardlink] {
        assert_refuses_and_preserves(&input, &["--emit=ast", "-o", alias.to_str().unwrap()]);
    }
}

#[test]
fn extensao_padrao_igual_a_entrada_e_recusada() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("extensao_padrao");
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("programa.ast");
    fs::write(&input, "fun main()\n    say(1)\n").unwrap();
    assert_refuses_and_preserves(&input, &["--emit=ast"]);
}
// pink-nav: end

// pink-nav: start
// pink-resume: --emit=semantic grava a árvore verificada; erro semântico falha; etapas anteriores não dependem dela.
// pink-keywords: teste, cli, emit, semantic, erro semântico, observabilidade
#[test]
fn emit_semantic_grava_a_arvore_verificada() {
    let input = source_file(
        "emit_semantic",
        "fun main()\n    let x: u8 = 1\n    say(x)\n",
    );
    let status = pinker()
        .arg(&input)
        .arg("--emit=semantic")
        .status()
        .unwrap();
    assert!(status.success());
    let content = fs::read_to_string(input.with_extension("semantic")).unwrap();
    assert!(content.contains("2:5          Let x#0: u8\n"), "{content}");
}

#[test]
fn erro_semantico_falha_mas_ast_e_tokens_continuam_visiveis() {
    let input = source_file("erro_semantico", "fun main()\n    break\n");
    let output = pinker().arg(&input).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ola.pk:2:5: erro: 'break' fora de um laço"),
        "{stderr}"
    );
    for stage in ["--emit=tokens", "--emit=ast"] {
        let output = pinker()
            .arg(&input)
            .args([stage, "-o", "-"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{stage}");
    }
}
// pink-nav: end
