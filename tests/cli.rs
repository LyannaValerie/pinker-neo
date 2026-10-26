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

/// Um diretório vazio só deste teste: nada de execuções anteriores sobra nele.
fn fresh_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Cria `<nome>/ola.pk` com o conteúdo dado, num diretório vazio, e devolve o caminho.
fn source_file(name: &str, content: &str) -> PathBuf {
    let dir = fresh_dir(name);
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
    let content = fs::read_to_string(input.with_extension("t")).unwrap();
    assert!(
        content.starts_with("# pinker tokens 1 · fonte: ")
            && content.contains("\n#POSIÇÃO TIPO           LEXEMA\n"),
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
    assert!(!input.with_extension("t").exists());
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
    let content = fs::read_to_string(input.with_extension("a")).unwrap();
    assert!(content.contains("1:1      Função main@1:5\n"), "{content}");
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
    assert!(!input.with_extension("a").exists());
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
    let symlink = dir.join("simbolico.a");
    let hardlink = dir.join("fisico.a");
    std::os::unix::fs::symlink(&input, &symlink).unwrap();
    fs::hard_link(&input, &hardlink).unwrap();
    for alias in [&symlink, &hardlink] {
        assert_refuses_and_preserves(&input, &["--emit=ast", "-o", alias.to_str().unwrap()]);
    }
}

#[test]
fn extensao_padrao_igual_a_entrada_e_recusada() {
    let dir = fresh_dir("extensao_padrao");
    let input = dir.join("programa.a");
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
    let content = fs::read_to_string(input.with_extension("s")).unwrap();
    assert!(
        content.contains("2:5          Let x#0@2:9: u8\n"),
        "{content}"
    );
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

// pink-nav: start
// pink-resume: Execução pela CLI: saída e código 0, falha de execução com código 3, --emit não executa, limite de recursão.
// pink-keywords: teste, cli, execução, código de saída, falha de execução, recursão, limite
#[test]
fn executa_o_programa_sem_emit() {
    let input = source_file("executa", "fun main()\n    say(\"olá\")\n    say(1 + 2)\n");
    let output = pinker().arg(&input).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "olá\n3\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn falha_de_execucao_termina_com_codigo_3() {
    let input = source_file(
        "falha_execucao",
        "fun main()\n    say(1)\n    let z = 0\n    say(1 / z)\n",
    );
    let output = pinker().arg(&input).output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "1\n");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ola.pk:4:11: erro de execução: divisão por zero"),
        "{stderr}"
    );
}

/// Executa com a saída padrão em `/dev/full`, onde toda escrita falha por falta de espaço.
/// A saída fica no buffer durante os `say`; a falha só aparece ao descarregá-la.
#[cfg(target_os = "linux")]
fn run_with_full_stdout(name: &str, source: &str) -> (Option<i32>, String) {
    let input = source_file(name, source);
    let full = fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .unwrap();
    let output = pinker().arg(&input).stdout(full).output().unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[cfg(target_os = "linux")]
#[test]
fn saida_que_nao_chega_ao_destino_e_falha_de_execucao() {
    let (code, stderr) = run_with_full_stdout("saida_cheia", "fun main()\n    say(1)\n");
    assert_eq!(code, Some(3), "{stderr}");
    assert!(
        stderr.contains("ola.pk:1:1: erro de execução: não foi possível escrever a saída"),
        "{stderr}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn falha_de_execucao_com_saida_perdida_mantem_o_diagnostico() {
    let (code, stderr) = run_with_full_stdout(
        "falha_e_saida_cheia",
        "fun main()\n    say(1)\n    let z = 0\n    say(1 / z)\n",
    );
    assert_eq!(code, Some(3), "{stderr}");
    assert!(
        stderr.contains("ola.pk:4:11: erro de execução: divisão por zero"),
        "{stderr}"
    );
    assert!(
        stderr.contains("pinker: além disso, a saída escrita antes da falha se perdeu"),
        "{stderr}"
    );
}

#[test]
fn emit_nao_executa_o_programa() {
    let input = source_file("emit_sem_execucao", "fun main()\n    say(\"executou\")\n");
    for stage in ["tokens", "ast", "semantic"] {
        let output = pinker()
            .arg(&input)
            .arg(format!("--emit={stage}"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{stage}");
        assert!(output.stdout.is_empty(), "{stage}");
    }
}

#[test]
fn recursao_profunda_ate_o_limite_e_alem_dele() {
    let program = |depth: usize| {
        format!(
            "fun desce(n: int) -> int\n    if n == 0\n        give 0\n    give 1 + desce(n - 1)\n\nfun main()\n    say(desce({depth}))\n"
        )
    };
    // A main conta como uma chamada: 9999 chamadas de desce completam o limite de 10000.
    let input = source_file("recursao_no_limite", &program(9998));
    let output = pinker().arg(&input).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "9998\n");

    let input = source_file("recursao_alem_do_limite", &program(100_000));
    let output = pinker().arg(&input).output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("limite de 10000 chamadas aninhadas excedido"),
        "{stderr}"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Pipeline por arquivos pela CLI: .pk -> .t -> .a -> .s -> execução, igual ao caminho direto, e seus erros.
// pink-keywords: teste, cli, pipeline, arquivo, .t, .a, .s, erro de formato, fonte
#[test]
fn pipeline_por_arquivos_e_igual_ao_direto() {
    let input = source_file(
        "pipeline_arquivos",
        "fun dobro(n: int) -> int\n    give n * 2\n\nfun main()\n    say(dobro(21))\n    say(\"fim\\n\")\n",
    );
    let dir = input.parent().unwrap().to_path_buf();
    for (from, stage) in [
        ("ola.pk", "tokens"),
        ("ola.t", "ast"),
        ("ola.a", "semantic"),
    ] {
        let status = pinker()
            .arg(dir.join(from))
            .arg(format!("--emit={stage}"))
            .status()
            .unwrap();
        assert!(status.success(), "{from} -> {stage}");
        // O arquivo produzido a partir do anterior é igual ao produzido direto do .pk.
        let direct = pinker()
            .arg(&input)
            .args([&format!("--emit={stage}"), "-o", "-"])
            .output()
            .unwrap();
        let extension = &stage[..1];
        let by_file = fs::read(input.with_extension(extension)).unwrap();
        assert_eq!(by_file, direct.stdout, "{stage}");
    }
    let by_file = pinker().arg(dir.join("ola.s")).output().unwrap();
    let direct = pinker().arg(&input).output().unwrap();
    assert_eq!(by_file.status.code(), Some(0));
    assert_eq!(by_file.stdout, direct.stdout);
    assert_eq!(String::from_utf8(by_file.stdout).unwrap(), "42\nfim\n\n");
}

#[test]
fn emit_de_etapa_anterior_a_entrada_e_erro_de_uso() {
    let input = source_file("etapa_anterior", "fun main()\n    say(1)\n");
    pinker().arg(&input).arg("--emit=ast").status().unwrap();
    let output = pinker()
        .arg(input.with_extension("a"))
        .arg("--emit=tokens")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("já é um arquivo da etapa ast"), "{stderr}");
}

#[test]
fn arquivo_malformado_aponta_a_propria_linha() {
    let input = source_file("arquivo_malformado", "fun main()\n    say(1)\n");
    let tokens = input.with_extension("t");
    fs::write(&tokens, "# pinker tokens 1 · fonte: ola.pk\n1:1      PALAVRA        fun\n1:5      COISA          x\n").unwrap();
    let output = pinker().arg(&tokens).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ola.t:3: erro de formato: tipo de token desconhecido 'COISA'"),
        "{stderr}"
    );
}

#[test]
fn erro_num_t_editado_aponta_o_codigo_fonte() {
    let input = source_file("t_editado", "fun main()\n    say(1)\n");
    pinker().arg(&input).arg("--emit=tokens").status().unwrap();
    let tokens = input.with_extension("t");
    // Sem a quebra depois do cabeçalho da função, o parser não encontra o bloco.
    let edited = fs::read_to_string(&tokens)
        .unwrap()
        .replacen("1:11     QUEBRA\n", "", 1);
    fs::write(&tokens, edited).unwrap();
    let output = pinker().arg(&tokens).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ola.pk:2:5: erro: esperava o fim da linha"),
        "{stderr}"
    );
}

#[test]
fn s_adulterado_e_recusado_antes_de_executar() {
    let input = source_file("s_adulterado", "fun main()\n    say(1)\n");
    pinker()
        .arg(&input)
        .arg("--emit=semantic")
        .status()
        .unwrap();
    let semantic = input.with_extension("s");
    let edited =
        fs::read_to_string(&semantic)
            .unwrap()
            .replacen("Inteiro 1: i64", "Inteiro 1: text", 1);
    fs::write(&semantic, edited).unwrap();
    let output = pinker().arg(&semantic).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ola.pk:2:9: erro: árvore inconsistente"),
        "{stderr}"
    );
}
// pink-nav: end
