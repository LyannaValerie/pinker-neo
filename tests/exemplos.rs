//! Os programas de `exemplos/` continuam executando, com a saída esperada, tanto direto
//! quanto pelo pipeline por arquivos, e a máquina virtual concorda com o interpretador de
//! árvore em cada um.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use pinker::bytecode;
use pinker::lexer::tokenize;
use pinker::lower::lower;
use pinker::parser::parse;
use pinker::semantic::check;
use pinker::{interpreter, vm};

// pink-nav: start
// pink-resume: Cada exemplo com a saída e o código de término esperados; todo .pk de exemplos/ precisa estar na lista.
// pink-keywords: teste, exemplos, saída esperada, código de saída, lista
/// O nome do exemplo, a saída padrão esperada e o código de término.
const EXAMPLES: &[(&str, &str, i32)] = &[
    ("ola", "maior que quatro\n", 0),
    (
        "fatorial",
        "1\n1\n2\n6\n24\n120\n720\n5040\n40320\n362880\n3628800\n39916800\n479001600\n\
         6227020800\n87178291200\n1307674368000\n20922789888000\n355687428096000\n\
         6402373705728000\n121645100408832000\n2432902008176640000\n",
        0,
    ),
    (
        "fibonacci",
        "0\n1\n1\n2\n3\n5\n8\n13\n21\n34\n55\n89\n144\n233\n377\n",
        0,
    ),
    ("collatz", "0\n8\n111\n118\n", 0),
    (
        "primos",
        "2\n3\n5\n7\n11\n13\n17\n19\n23\n29\n31\n37\n41\n43\n47\nprimos abaixo de 50:\n15\n",
        0,
    ),
    (
        "fizzbuzz",
        "1\n2\nFizz\n4\nBuzz\nFizz\n7\n8\nFizz\nBuzz\n11\nFizz\n13\n14\nFizzBuzz\n",
        0,
    ),
    (
        "tipos",
        "255\n160\n-128\n18446744073709551615\n1255\n200\n",
        0,
    ),
    ("sombra", "1\ndentro do if\nfalse\ndentro do if\n1\n", 0),
    // Termina de propósito com uma falha de execução.
    ("transbordo", "250\n251\n252\n253\n254\n255\n", 3),
];

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("exemplos")
}

#[test]
fn todo_exemplo_esta_na_lista() {
    let mut found: Vec<String> = fs::read_dir(examples_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "pk"))
        .map(|path| path.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    found.sort();
    let mut listed: Vec<String> = EXAMPLES.iter().map(|(name, ..)| name.to_string()).collect();
    listed.sort();
    assert_eq!(found, listed);
}
// pink-nav: end

// pink-nav: start
// pink-resume: Executa cada exemplo direto e pelo pipeline por arquivos (.t, .a, .s, .b) e compara a saída e o diagnóstico; a VM concorda com o interpretador.
// pink-keywords: teste, exemplos, pipeline, arquivo, execução, equivalência, bytecode, vm, interpretador, paridade
fn pinker(args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pinker"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn exemplos_executam_direto_e_por_arquivos() {
    for &(name, expected, code) in EXAMPLES {
        // Uma cópia num diretório só do teste: os arquivos .t, .a e .s ficam fora de
        // exemplos/.
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("exemplo_{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join(format!("{name}.pk"));
        fs::copy(examples_dir().join(format!("{name}.pk")), &source).unwrap();

        let direct = pinker(&[&source]);
        assert_eq!(direct.status.code(), Some(code), "{name}: {direct:?}");
        assert_eq!(String::from_utf8_lossy(&direct.stdout), expected, "{name}");

        for (from, stage) in [
            ("pk", "tokens"),
            ("t", "ast"),
            ("a", "semantic"),
            ("s", "bytecode"),
        ] {
            let emit = format!("--emit={stage}");
            let output = pinker(&[&source.with_extension(from), Path::new(&emit)]);
            assert!(
                output.status.success(),
                "{name}: {from} -> {stage}: {output:?}"
            );
        }
        // Tanto o .s quanto o .b executam, e igual ao caminho direto.
        for extension in ["s", "b"] {
            let by_file = pinker(&[&source.with_extension(extension)]);
            assert_eq!(
                by_file.status.code(),
                direct.status.code(),
                "{name}.{extension}"
            );
            assert_eq!(by_file.stdout, direct.stdout, "{name}.{extension}");
            assert_eq!(by_file.stderr, direct.stderr, "{name}.{extension}");
        }
    }
}

#[test]
fn vm_e_interpretador_concordam_nos_exemplos() {
    for &(name, ..) in EXAMPLES {
        let source = fs::read_to_string(examples_dir().join(format!("{name}.pk"))).unwrap();
        let program = check(&parse(&tokenize(&source).unwrap()).unwrap()).unwrap();
        let lowered = lower(&program);
        bytecode::verify(&lowered).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        // O .b gravado e lido de volta é o mesmo bytecode.
        let content = bytecode::render(&lowered, "x.pk");
        assert_eq!(bytecode::read(&content).unwrap(), lowered, "{name}");

        let mut by_tree = Vec::new();
        let tree_result = interpreter::run(&program, &mut by_tree);
        let mut by_vm = Vec::new();
        let vm_result = vm::run(&lowered, &mut by_vm);
        assert_eq!(by_vm, by_tree, "{name}");
        assert_eq!(vm_result, tree_result, "{name}");
    }
}
// pink-nav: end
