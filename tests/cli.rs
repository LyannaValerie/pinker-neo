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
    let output = pinker().arg(&input).arg("--emit=ast").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("etapa desconhecida 'ast'"), "{stderr}");
}
// pink-nav: end
