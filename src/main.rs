//! `pinker`: a linha de comando do compilador.

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs, thread};

use pinker::diagnostic::Diagnostic;
use pinker::{ast, checked, interpreter, lexer, parser, semantic, token};

const USAGE: &str = "uso: pinker <arquivo.pk> [--emit=<etapa>] [-o <saída>]

  --emit=<etapa>  grava o resultado de uma etapa do pipeline
                  etapas disponíveis: tokens, ast, semantic
  -o <saída>      caminho do arquivo gravado (padrão: o arquivo de entrada com a
                  extensão da etapa, como ola.tokens); '-' escreve na saída padrão
  -h, --help      mostra esta ajuda

Sem --emit, o programa passa por todas as etapas e é executado.

Códigos de saída: 0 sucesso, 1 erro de compilação ou de arquivo, 2 uso incorreto,
3 falha de execução.";

// pink-nav: start
// pink-resume: Opções da linha de comando e as etapas do pipeline que --emit pode gravar.
// pink-keywords: cli, argumentos, opções, emit, etapa, saída, -o
enum Stage {
    Tokens,
    Ast,
    Semantic,
}

impl Stage {
    fn from_name(name: &str) -> Option<Stage> {
        match name {
            "tokens" => Some(Stage::Tokens),
            "ast" => Some(Stage::Ast),
            "semantic" => Some(Stage::Semantic),
            _ => None,
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            Stage::Tokens => "tokens",
            Stage::Ast => "ast",
            Stage::Semantic => "semantic",
        }
    }
}

struct Options {
    input: PathBuf,
    emit: Option<Stage>,
    output: Option<String>,
}

/// Lê os argumentos. `Ok(None)` significa que a ajuda foi pedida.
fn parse_args(args: &[String]) -> Result<Option<Options>, String> {
    let mut input = None;
    let mut emit = None;
    let mut output = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "-h" || arg == "--help" {
            return Ok(None);
        } else if let Some(name) = arg.strip_prefix("--emit=") {
            let stage = Stage::from_name(name).ok_or_else(|| {
                format!("etapa desconhecida '{name}'; etapas disponíveis: tokens, ast, semantic")
            })?;
            emit = Some(stage);
        } else if arg == "-o" {
            let path = args.next().ok_or("'-o' precisa de um caminho")?;
            output = Some(path.clone());
        } else if arg.starts_with('-') {
            return Err(format!("opção desconhecida '{arg}'"));
        } else if input.is_some() {
            return Err(format!("mais de um arquivo de entrada: '{arg}'"));
        } else {
            input = Some(PathBuf::from(arg));
        }
    }
    let input = input.ok_or("falta o arquivo de entrada")?;
    if output.is_some() && emit.is_none() {
        return Err("'-o' só faz sentido junto com --emit".to_string());
    }
    Ok(Some(Options {
        input,
        emit,
        output,
    }))
}
// pink-nav: end

// pink-nav: start
// pink-resume: Ponto de entrada: lê o arquivo, roda as etapas até a pedida, executa o programa e define o código de saída.
// pink-keywords: main, execução, erro, código de saída, gravar arquivo, proteção da entrada, pilha
/// Pilha da thread que roda o pipeline. O interpretador é recursivo, e esta pilha foi
/// validada nos cenários testados (até `MAX_CALL_DEPTH` chamadas aninhadas, em debug e
/// release); não é uma garantia geral contra estouro.
const STACK_SIZE: usize = 256 * 1024 * 1024;

fn main() -> ExitCode {
    let thread = match thread::Builder::new().stack_size(STACK_SIZE).spawn(pinker) {
        Ok(thread) => thread,
        Err(e) => {
            eprintln!("pinker: não foi possível criar a thread de execução: {e}");
            return ExitCode::FAILURE;
        }
    };
    // Se a thread terminar em pânico (um defeito do próprio pinker), a mensagem do pânico
    // já foi escrita; o código 101 é o mesmo que o Rust usa nesse caso.
    ExitCode::from(thread.join().unwrap_or(101))
}

fn pinker() -> u8 {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return 0;
        }
        Err(message) => {
            eprintln!("pinker: {message}\n\n{USAGE}");
            return 2;
        }
    };
    match run(&options) {
        Ok(()) => 0,
        Err(failure) => {
            eprintln!("{}", failure.message);
            failure.code
        }
    }
}

/// Uma falha com a mensagem e o código de saída correspondente.
struct Failure {
    message: String,
    code: u8,
}

/// Erros de compilação e de arquivos terminam com código 1.
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure { message, code: 1 }
    }
}

/// Roda as etapas em ordem e para na etapa pedida por `--emit`, se houver. Sem
/// `--emit`, executa o programa.
fn run(options: &Options) -> Result<(), Failure> {
    let path = &options.input;
    let output = options.output.as_deref();
    let source = fs::read_to_string(path)
        .map_err(|e| format!("pinker: não foi possível ler '{}': {e}", path.display()))?;

    let tokens = lexer::tokenize(&source).map_err(|e| report(path, "erro", e))?;
    if let Some(stage @ Stage::Tokens) = &options.emit {
        return Ok(write_output(path, stage, output, &token::render(&tokens))?);
    }

    let program = parser::parse(&tokens).map_err(|e| report(path, "erro", e))?;
    if let Some(stage @ Stage::Ast) = &options.emit {
        return Ok(write_output(path, stage, output, &ast::render(&program))?);
    }

    let checked = semantic::check(&program).map_err(|e| report(path, "erro", e))?;
    if let Some(stage @ Stage::Semantic) = &options.emit {
        return Ok(write_output(
            path,
            stage,
            output,
            &checked::render(&checked),
        )?);
    }

    let mut stdout = BufWriter::new(io::stdout().lock());
    let result = interpreter::run(&checked, &mut stdout);
    // A saída escrita antes de uma falha aparece antes do diagnóstico.
    let _ = stdout.flush();
    result.map_err(|e| Failure {
        message: report(path, "erro de execução", e),
        code: 3,
    })
}

/// Formata um diagnóstico como `arquivo:linha:coluna: <tipo>: mensagem`.
fn report(path: &Path, kind: &str, diagnostic: Diagnostic) -> String {
    format!(
        "{}:{}:{}: {kind}: {}",
        path.display(),
        diagnostic.position.line,
        diagnostic.position.column,
        diagnostic.message
    )
}

fn write_output(
    input: &Path,
    stage: &Stage,
    output: Option<&str>,
    content: &str,
) -> Result<(), String> {
    if output == Some("-") {
        print!("{content}");
        return Ok(());
    }
    let target = match output {
        Some(path) => PathBuf::from(path),
        None => input.with_extension(stage.extension()),
    };
    if same_file(input, &target) {
        return Err(format!(
            "pinker: a saída '{}' é o próprio arquivo de entrada; escolha outro caminho com -o",
            target.display()
        ));
    }
    fs::write(&target, content).map_err(|e| {
        format!(
            "pinker: não foi possível gravar '{}': {e}",
            target.display()
        )
    })?;
    eprintln!(
        "pinker: etapa '{}' gravada em '{}'",
        stage.extension(),
        target.display()
    );
    Ok(())
}

/// Verdadeiro quando os dois caminhos levam ao mesmo arquivo, inclusive por links
/// simbólicos ou físicos. Um caminho que ainda não existe nunca é a entrada.
#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_file(a: &Path, b: &Path) -> bool {
    matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(a), Ok(b)) if a == b)
}
// pink-nav: end
