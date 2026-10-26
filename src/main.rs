//! `pinker`: a linha de comando do compilador.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{env, fs};

use pinker::diagnostic::Diagnostic;
use pinker::{ast, lexer, parser, token};

const USAGE: &str = "uso: pinker <arquivo.pk> [--emit=<etapa>] [-o <saída>]

  --emit=<etapa>  grava o resultado de uma etapa do pipeline
                  etapas disponíveis: tokens, ast
  -o <saída>      caminho do arquivo gravado (padrão: o arquivo de entrada com a
                  extensão da etapa, como ola.tokens); '-' escreve na saída padrão
  -h, --help      mostra esta ajuda

Sem --emit, o arquivo passa pelas etapas disponíveis e só os erros são mostrados.";

// pink-nav: start
// pink-resume: Opções da linha de comando e as etapas do pipeline que --emit pode gravar.
// pink-keywords: cli, argumentos, opções, emit, etapa, saída, -o
enum Stage {
    Tokens,
    Ast,
}

impl Stage {
    fn from_name(name: &str) -> Option<Stage> {
        match name {
            "tokens" => Some(Stage::Tokens),
            "ast" => Some(Stage::Ast),
            _ => None,
        }
    }

    fn extension(&self) -> &'static str {
        match self {
            Stage::Tokens => "tokens",
            Stage::Ast => "ast",
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
                format!("etapa desconhecida '{name}'; etapas disponíveis: tokens, ast")
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
// pink-resume: Ponto de entrada: lê o arquivo, roda as etapas até a pedida, mostra erros e grava o resultado.
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

/// Roda as etapas em ordem e para na etapa pedida por `--emit`, se houver.
fn run(options: &Options) -> Result<(), String> {
    let path = &options.input;
    let output = options.output.as_deref();
    let source = fs::read_to_string(path)
        .map_err(|e| format!("pinker: não foi possível ler '{}': {e}", path.display()))?;

    let tokens = lexer::tokenize(&source).map_err(|e| report(path, e))?;
    if let Some(stage @ Stage::Tokens) = &options.emit {
        return write_output(path, stage, output, &token::render(&tokens));
    }

    let program = parser::parse(&tokens).map_err(|e| report(path, e))?;
    if let Some(stage @ Stage::Ast) = &options.emit {
        return write_output(path, stage, output, &ast::render(&program));
    }

    eprintln!(
        "pinker: '{}' passou pelo lexer e pelo parser sem erros (a execução ainda não existe)",
        path.display()
    );
    Ok(())
}

/// Formata um diagnóstico como `arquivo:linha:coluna: erro: mensagem`.
fn report(path: &Path, diagnostic: Diagnostic) -> String {
    format!(
        "{}:{}:{}: erro: {}",
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
// pink-nav: end
