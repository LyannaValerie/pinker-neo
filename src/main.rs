//! `pinker`: a linha de comando do compilador.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

use pinker::{lexer, token};

const USAGE: &str = "uso: pinker <arquivo.pk> [--emit=<etapa>] [-o <saída>]

  --emit=<etapa>  grava o resultado de uma etapa do pipeline
                  etapas disponíveis: tokens
  -o <saída>      caminho do arquivo gravado (padrão: o arquivo de entrada com a
                  extensão da etapa, como ola.tokens); '-' escreve na saída padrão
  -h, --help      mostra esta ajuda

Sem --emit, o arquivo passa pelas etapas disponíveis e só os erros são mostrados.";

// pink-nav: start
// pink-resume: Opções da linha de comando e as etapas do pipeline que --emit pode gravar.
// pink-keywords: cli, argumentos, opções, emit, etapa, saída, -o
enum Stage {
    Tokens,
}

impl Stage {
    fn from_name(name: &str) -> Option<Stage> {
        match name {
            "tokens" => Some(Stage::Tokens),
            _ => None,
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            Stage::Tokens => "tokens",
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
                format!("etapa desconhecida '{name}'; etapas disponíveis: tokens")
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
// pink-resume: Ponto de entrada: lê o arquivo, roda o lexer, mostra erros e grava a etapa pedida.
// pink-keywords: main, execução, erro, código de saída, gravar arquivo
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let options = match parse_args(&args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("pinker: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: &Options) -> Result<(), String> {
    let path = &options.input;
    let source = fs::read_to_string(path)
        .map_err(|e| format!("pinker: não foi possível ler '{}': {e}", path.display()))?;

    let tokens = lexer::tokenize(&source).map_err(|e| {
        format!(
            "{}:{}:{}: erro: {}",
            path.display(),
            e.position.line,
            e.position.column,
            e.message
        )
    })?;

    let Some(stage) = &options.emit else {
        eprintln!(
            "pinker: '{}' passou pelo lexer sem erros (a execução ainda não existe)",
            path.display()
        );
        return Ok(());
    };
    let content = match stage {
        Stage::Tokens => token::render(&tokens),
    };
    write_output(path, stage, options.output.as_deref(), &content)
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
    fs::write(&target, content).map_err(|e| {
        format!(
            "pinker: não foi possível gravar '{}': {e}",
            target.display()
        )
    })?;
    eprintln!(
        "pinker: {} gravados em '{}'",
        stage.extension(),
        target.display()
    );
    Ok(())
}
// pink-nav: end
