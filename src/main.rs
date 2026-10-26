//! `pinker`: a linha de comando do compilador.

use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs, thread};

use pinker::artifact::{self, Format, FormatError};
use pinker::diagnostic::Diagnostic;
use pinker::{ast, checked, interpreter, lexer, parser, semantic, token, verifier};

const USAGE: &str = "uso: pinker <arquivo> [--emit=<etapa>] [-o <saída>]

  <arquivo>       código-fonte (.pk) ou o arquivo de uma etapa (.t, .a ou .s); o
                  pipeline começa na etapa seguinte à do arquivo
  --emit=<etapa>  grava o resultado de uma etapa do pipeline e para nela
                  etapas: tokens (.t), ast (.a), semantic (.s)
  -o <saída>      caminho do arquivo gravado (padrão: o arquivo de entrada com a
                  extensão da etapa, como ola.t); '-' escreve na saída padrão
  -h, --help      mostra esta ajuda

Sem --emit, o programa passa pelas etapas restantes e é executado:
  pinker ola.pk --emit=tokens     # ola.pk -> ola.t
  pinker ola.t --emit=ast         # ola.t  -> ola.a
  pinker ola.a --emit=semantic    # ola.a  -> ola.s
  pinker ola.s                    # executa

Códigos de saída: 0 sucesso, 1 erro de compilação ou de arquivo, 2 uso incorreto,
3 falha de execução.";

// pink-nav: start
// pink-resume: Opções da linha de comando: o arquivo de entrada, a etapa de --emit e o destino de -o.
// pink-keywords: cli, argumentos, opções, emit, etapa, saída, -o
struct Options {
    input: PathBuf,
    emit: Option<Format>,
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
            let format = Format::from_name(name).ok_or_else(|| {
                format!("etapa desconhecida '{name}'; etapas disponíveis: tokens, ast, semantic")
            })?;
            emit = Some(format);
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
// pink-resume: Ponto de entrada: thread com pilha ampliada, códigos de saída e o tipo de falha com o seu código.
// pink-keywords: main, thread, pilha, código de saída, falha
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
// pink-nav: end

// pink-nav: start
// pink-resume: O pipeline: reconhece a entrada pelo cabeçalho, avança etapa por etapa, grava a pedida ou executa.
// pink-keywords: pipeline, etapa, entrada, cabeçalho, tokens, ast, semantic, executar, verificador
/// O que o pipeline tem em mãos depois de cada etapa.
enum Data {
    Source,
    Tokens(Vec<token::Token>),
    Ast(ast::Program),
    Semantic(checked::CheckedProgram),
}

/// Roda as etapas a partir da entrada e para na pedida por `--emit`, se houver. Sem
/// `--emit`, executa o programa.
fn run(options: &Options) -> Result<(), Failure> {
    let path = &options.input;
    let content = fs::read_to_string(path)
        .map_err(|e| format!("pinker: não foi possível ler '{}': {e}", path.display()))?;
    let malformed = |e: FormatError| {
        format!(
            "{}:{}: erro de formato: {}",
            path.display(),
            e.line,
            e.message
        )
    };

    // Um arquivo de etapa diz no cabeçalho de que código-fonte veio; os diagnósticos da
    // linguagem apontam para ele.
    let (start, source) = match artifact::detect(&content).map_err(malformed)? {
        Some((format, source)) => (Some(format), source),
        None => (None, path.display().to_string()),
    };
    if let (Some(start), Some(emit)) = (start, options.emit)
        && emit <= start
    {
        return Err(Failure {
            message: format!(
                "pinker: a entrada já é um arquivo da etapa {}; --emit precisa de uma etapa posterior",
                start.name()
            ),
            code: 2,
        });
    }
    let failed = |e: Diagnostic| report(&source, "erro", e);
    let mut data = match start {
        None => Data::Source,
        Some(Format::Tokens) => Data::Tokens(token::read(&content).map_err(malformed)?),
        Some(Format::Ast) => Data::Ast(ast::read(&content).map_err(malformed)?),
        Some(Format::Semantic) => {
            let program = checked::read(&content).map_err(malformed)?;
            // Depois do .s nada mais confere o programa: o verificador garante que a
            // árvore respeita o que o interpretador supõe.
            verifier::verify(&program).map_err(failed)?;
            Data::Semantic(program)
        }
    };

    let output = options.output.as_deref();
    let emit = |format: Format, content: String| -> Result<(), Failure> {
        Ok(write_output(path, format, output, &content)?)
    };
    loop {
        data = match data {
            Data::Source => Data::Tokens(lexer::tokenize(&content).map_err(failed)?),
            Data::Tokens(tokens) => {
                if options.emit == Some(Format::Tokens) {
                    return emit(Format::Tokens, token::render(&tokens, &source));
                }
                Data::Ast(parser::parse(&tokens).map_err(failed)?)
            }
            Data::Ast(program) => {
                if options.emit == Some(Format::Ast) {
                    return emit(Format::Ast, ast::render(&program, &source));
                }
                Data::Semantic(semantic::check(&program).map_err(failed)?)
            }
            Data::Semantic(program) => {
                if options.emit == Some(Format::Semantic) {
                    return emit(Format::Semantic, checked::render(&program, &source));
                }
                return execute(&program, &source);
            }
        };
    }
}

fn execute(program: &checked::CheckedProgram, source: &str) -> Result<(), Failure> {
    let mut stdout = BufWriter::new(io::stdout().lock());
    let result = interpreter::run(program, &mut stdout);
    // A saída escrita antes de uma falha aparece antes do diagnóstico.
    let _ = stdout.flush();
    result.map_err(|e| Failure {
        message: report(source, "erro de execução", e),
        code: 3,
    })
}

/// Formata um diagnóstico como `fonte:linha:coluna: <tipo>: mensagem`.
fn report(source: &str, kind: &str, diagnostic: Diagnostic) -> String {
    format!(
        "{source}:{}:{}: {kind}: {}",
        diagnostic.position.line, diagnostic.position.column, diagnostic.message
    )
}
// pink-nav: end

// pink-nav: start
// pink-resume: Grava o resultado de uma etapa, recusando sobrescrever o próprio arquivo de entrada.
// pink-keywords: gravar arquivo, saída, -o, proteção da entrada, symlink, hard link
fn write_output(
    input: &Path,
    format: Format,
    output: Option<&str>,
    content: &str,
) -> Result<(), String> {
    if output == Some("-") {
        print!("{content}");
        return Ok(());
    }
    let target = match output {
        Some(path) => PathBuf::from(path),
        None => input.with_extension(format.extension()),
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
        format.name(),
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
