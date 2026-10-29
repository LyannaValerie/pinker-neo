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
    // A tabela de símbolos guarda o tipo, a posição e a espécie; a árvore só o número.
    assert!(
        content.contains("[variáveis main]\n#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE\n0   x     u8    2:9         let\n"),
        "{content}"
    );
    assert!(content.contains("2:5          Let x#0\n"), "{content}");
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
    let lines: Vec<&str> = stderr.lines().collect();
    // A falha da saída é o próprio diagnóstico, com o trecho da main: não aparece de novo
    // como saída perdida.
    assert_eq!(lines.len(), 3, "{stderr}");
    assert!(
        lines[0].contains("ola.pk:1:1: erro de execução: não foi possível escrever a saída"),
        "{stderr}"
    );
    assert_eq!(lines[1..], ["  1 | fun main()", "    | ^"], "{stderr}");
}

#[cfg(target_os = "linux")]
#[test]
fn falha_ao_escrever_no_say_aparece_uma_vez() {
    // A saída passa do tamanho do buffer, e a escrita falha durante um say.
    let (code, stderr) = run_with_full_stdout(
        "say_cheio",
        "fun main()\n    let mut i = 0\n    while i < 10000\n        say(i)\n        i = i + 1\n",
    );
    assert_eq!(code, Some(3), "{stderr}");
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 3, "{stderr}");
    assert!(
        lines[0].contains("ola.pk:4:9: erro de execução: não foi possível escrever a saída"),
        "{stderr}"
    );
    assert_eq!(
        lines[1..],
        ["  4 |         say(i)", "    |         ^"],
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
    let lines: Vec<&str> = stderr.lines().collect();
    // O diagnóstico e o trecho vêm primeiro; o aviso da saída perdida, uma vez, depois.
    assert_eq!(lines.len(), 4, "{stderr}");
    assert!(
        lines[0].ends_with("ola.pk:4:11: erro de execução: divisão por zero"),
        "{stderr}"
    );
    assert_eq!(
        lines[1..3],
        ["  4 |     say(1 / z)", "    |           ^"],
        "{stderr}"
    );
    assert!(
        lines[3].starts_with("pinker: além disso, a saída escrita antes da falha se perdeu"),
        "{stderr}"
    );
}

#[test]
fn emit_nao_executa_o_programa() {
    let input = source_file("emit_sem_execucao", "fun main()\n    say(\"executou\")\n");
    for stage in ["tokens", "ast", "semantic", "bytecode"] {
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

// pink-nav: start
// pink-resume: --emit=bytecode grava o .b; um .b executa na VM; um .b adulterado é recusado antes de executar; o .b não volta a etapas anteriores.
// pink-keywords: teste, cli, emit, bytecode, arquivo .b, vm, verificador, adulterado
#[test]
fn emit_bytecode_grava_o_b_que_executa() {
    let input = source_file(
        "emit_bytecode",
        "fun main()\n    let x: u8 = 1\n    say(x)\n",
    );
    let status = pinker()
        .arg(&input)
        .arg("--emit=bytecode")
        .status()
        .unwrap();
    assert!(status.success());
    let compiled = input.with_extension("b");
    let content = fs::read_to_string(&compiled).unwrap();
    assert!(
        content.starts_with("# pinker bytecode 1 · fonte: "),
        "{content}"
    );
    assert!(content.contains("[código main 1:1]\n"), "{content}");
    assert!(content.contains("store x#0\n"), "{content}");

    let output = pinker().arg(&compiled).output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "1\n");

    let output = pinker()
        .arg(&compiled)
        .arg("--emit=semantic")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn b_adulterado_e_recusado_antes_de_executar() {
    let input = source_file("b_adulterado", "fun main()\n    say(1)\n    say(2)\n");
    pinker()
        .arg(&input)
        .arg("--emit=bytecode")
        .status()
        .unwrap();
    let compiled = input.with_extension("b");
    let edited = fs::read_to_string(&compiled)
        .unwrap()
        .replacen("say.i64", "say.text", 1);
    fs::write(&compiled, edited).unwrap();
    let output = pinker().arg(&compiled).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("ola.pk:2:5: erro: bytecode inconsistente"),
        "{stderr}"
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Trecho do código nos diagnósticos: só com entrada de código-fonte (pelo conteúdo), nunca a partir da fonte declarada por um arquivo de etapa.
// pink-keywords: teste, cli, diagnóstico, trecho, marcador, unicode, crlf, tabulação, controle, fim do arquivo, artefato, fonte
/// Executa `pinker <entrada> [args]` e devolve o código, a saída padrão e o stderr.
fn run(input: &PathBuf, args: &[&str]) -> (Option<i32>, String, String) {
    let output = pinker().arg(input).args(args).output().unwrap();
    (
        output.status.code(),
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn erros_de_compilacao_mostram_a_linha_e_o_marcador() {
    let cases = [
        (
            "trecho_lexico",
            "fun main()\n    say(\"ação\") @\n",
            ":2:17: erro: caractere inesperado '@'\n  2 |     say(\"ação\") @\n    |                 ^\n",
        ),
        (
            "trecho_sintatico",
            "fun main()\n    a + b\n",
            ":2:7: erro: esperava '=' ou '(' depois de 'a' (uma instrução que começa com um nome é uma atribuição ou uma chamada), encontrei '+'\n  2 |     a + b\n    |       ^\n",
        ),
        (
            "trecho_semantico",
            "fun main()\n    break\n",
            ":2:5: erro: 'break' fora de um laço\n  2 |     break\n    |     ^\n",
        ),
        (
            "trecho_crlf_e_tabulacao",
            "fun main()\r\n\tsay(1)\r\n",
            ":2:1: erro: tabulação na indentação; use espaços\n  2 |     say(1)\n    | ^\n",
        ),
        (
            "trecho_controle",
            "fun main()\n    say(\"\u{1b}[2J\") @\n",
            ":2:17: erro: caractere inesperado '@'\n  2 |     say(\"\\u{1b}[2J\") @\n    |                      ^\n",
        ),
        (
            "trecho_fim_com_quebra",
            "fun main()\n",
            ":2:1: erro: esperava um bloco indentado, encontrei o fim do arquivo\n  2 | \n    | ^\n",
        ),
        (
            "trecho_fim_sem_quebra",
            "fun main()\n    say(1",
            ":2:8: erro: '(' aberto aqui não foi fechado\n  2 |     say(1\n    |        ^\n",
        ),
    ];
    for (name, source, expected) in cases {
        let input = source_file(name, source);
        let (code, stdout, stderr) = run(&input, &[]);
        assert_eq!(code, Some(1), "{name}: {stderr}");
        assert!(stdout.is_empty(), "{name}");
        assert_eq!(stderr, format!("{}{expected}", input.display()), "{name}");
    }
}

#[test]
fn falha_de_execucao_mostra_o_trecho_depois_da_saida() {
    let input = source_file(
        "trecho_execucao",
        "fun main()\n    say(1)\n    let z = 0\n    say(1 / z)\n",
    );
    let (code, stdout, stderr) = run(&input, &[]);
    assert_eq!(code, Some(3), "{stderr}");
    assert_eq!(stdout, "1\n");
    assert_eq!(
        stderr,
        format!(
            "{}:4:11: erro de execução: divisão por zero\n  4 |     say(1 / z)\n    |           ^\n",
            input.display()
        )
    );
}

/// Grava o `.pk` com outro conteúdo, de várias linhas, depois que as etapas foram geradas:
/// se o diagnóstico abrisse a fonte declarada, haveria uma linha para mostrar.
fn replace_source(input: &PathBuf) {
    fs::write(
        input,
        "outro conteúdo\nque não é o programa\nnem o que\nfoi compilado\n",
    )
    .unwrap();
}

#[test]
fn arquivo_de_etapa_nao_mostra_a_fonte_declarada() {
    // Erro semântico encontrado ao continuar de um .t ou de um .a.
    let input = source_file("fonte_trocada_compilacao", "fun main()\n    break\n");
    for stage in ["tokens", "ast"] {
        let status = pinker().arg(&input).arg(format!("--emit={stage}")).status();
        assert!(status.unwrap().success(), "{stage}");
    }
    replace_source(&input);
    for extension in ["t", "a"] {
        let (code, stdout, stderr) = run(&input.with_extension(extension), &[]);
        assert_eq!(code, Some(1), "{extension}: {stderr}");
        assert!(stdout.is_empty(), "{extension}");
        assert_eq!(
            stderr,
            format!("{}:2:5: erro: 'break' fora de um laço\n", input.display()),
            "{extension}"
        );
    }

    // Falha de execução ao continuar de um .s ou de um .b.
    let input = source_file(
        "fonte_trocada_execucao",
        "fun main()\n    say(1)\n    let z = 0\n    say(1 / z)\n",
    );
    for stage in ["semantic", "bytecode"] {
        let status = pinker().arg(&input).arg(format!("--emit={stage}")).status();
        assert!(status.unwrap().success(), "{stage}");
    }
    replace_source(&input);
    for extension in ["s", "b"] {
        let (code, stdout, stderr) = run(&input.with_extension(extension), &[]);
        assert_eq!(code, Some(3), "{extension}: {stderr}");
        assert_eq!(stdout, "1\n", "{extension}");
        assert_eq!(
            stderr,
            format!(
                "{}:4:11: erro de execução: divisão por zero\n",
                input.display()
            ),
            "{extension}"
        );
    }
}

#[test]
fn tipo_de_entrada_vem_do_conteudo_e_nao_da_extensao() {
    // Código-fonte num arquivo com a extensão do bytecode: mostra o trecho.
    let dir = fresh_dir("conteudo_fonte_com_extensao_b");
    let disguised = dir.join("programa.b");
    fs::write(&disguised, "fun main()\n    break\n").unwrap();
    let (code, _, stderr) = run(&disguised, &[]);
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(
        stderr,
        format!(
            "{}:2:5: erro: 'break' fora de um laço\n  2 |     break\n    |     ^\n",
            disguised.display()
        )
    );

    // Tokens num arquivo .pk: é um arquivo de etapa, sem trecho, mesmo com a fonte
    // declarada intacta.
    let input = source_file("conteudo_tokens_com_extensao_pk", "fun main()\n    break\n");
    let tokens = input.with_file_name("tokens.pk");
    let status = pinker()
        .arg(&input)
        .args(["--emit=tokens", "-o"])
        .arg(&tokens)
        .status();
    assert!(status.unwrap().success());
    let (code, _, stderr) = run(&tokens, &[]);
    assert_eq!(code, Some(1), "{stderr}");
    assert_eq!(
        stderr,
        format!("{}:2:5: erro: 'break' fora de um laço\n", input.display())
    );
}
// pink-nav: end

// pink-nav: start
// pink-resume: Operando ausente pela CLI: diagnóstico com trecho, código 1 sem executar; --emit=tokens funciona e um --emit=ast que falha não cria nem sobrescreve o .a.
// pink-keywords: teste, cli, operando ausente, operador, diagnóstico, emit, tokens, ast, sobrescrever
#[test]
fn operando_ausente_mostra_o_operador_e_nao_executa() {
    for (name, source, expected) in [
        (
            "operando_ausente_esquerda",
            "fun main()\n    say(\"executou\")\n    say(* 2)\n",
            ":3:9: erro: falta o operando à esquerda de '*'\n  3 |     say(* 2)\n    |         ^\n",
        ),
        (
            "operando_ausente_direita",
            "fun main()\n    say(\"executou\")\n    let x = 1 +\n",
            ":3:16: erro: falta o operando à direita de '+', encontrei o fim da linha\n  3 |     let x = 1 +\n    |                ^\n",
        ),
    ] {
        let input = source_file(name, source);
        let (code, stdout, stderr) = run(&input, &[]);
        assert_eq!(code, Some(1), "{name}: {stderr}");
        assert!(stdout.is_empty(), "{name}: {stdout}");
        assert_eq!(stderr, format!("{}{expected}", input.display()), "{name}");
    }
}

#[test]
fn erro_do_parser_nao_impede_tokens_nem_grava_a_arvore() {
    let input = source_file("operando_ausente_emit", "fun main()\n    say(1 *)\n");
    let (code, stdout, _) = run(&input, &["--emit=tokens", "-o", "-"]);
    assert_eq!(code, Some(0));
    assert!(stdout.contains("2:11     SÍMBOLO        *\n"), "{stdout}");

    // Sem .a anterior, nada é criado; com um, ele fica intacto.
    let tree = input.with_extension("a");
    let (code, _, stderr) = run(&input, &["--emit=ast"]);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(!tree.exists());
    fs::write(&tree, "conteúdo anterior\n").unwrap();
    let (code, _, stderr) = run(&input, &["--emit=ast"]);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.contains("erro: falta o operando à direita de '*'"),
        "{stderr}"
    );
    assert_eq!(fs::read_to_string(&tree).unwrap(), "conteúdo anterior\n");
}
// pink-nav: end

// pink-nav: start
// pink-resume: Falha aninhada pela CLI: código 3, só os efeitos anteriores no stdout, o diagnóstico da operação interna com trecho, uma vez.
// pink-keywords: teste, cli, falha de execução, aninhada, operando, efeito, stdout, stderr, código 3
#[test]
fn falha_dentro_de_um_operando_interrompe_a_expressao() {
    let input = source_file(
        "falha_aninhada",
        "fun marca(t: text, n: int) -> int\n    say(t)\n    give n\n\nfun main()\n    say(\"antes\")\n    say(marca(\"a\", 1) + marca(\"b\", 6) / marca(\"c\", 0))\n    say(\"depois\")\n",
    );
    let (code, stdout, stderr) = run(&input, &[]);
    assert_eq!(code, Some(3), "{stderr}");
    // As marcas da soma e da divisão, sem o resultado da soma nem a linha seguinte.
    assert_eq!(stdout, "antes\na\nb\nc\n");
    assert_eq!(
        stderr,
        format!(
            "{}:7:39: erro de execução: divisão por zero\n  7 |     say(marca(\"a\", 1) + marca(\"b\", 6) / marca(\"c\", 0))\n    |                                       ^\n",
            input.display()
        )
    );
}
// pink-nav: end
