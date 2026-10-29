//! Medição de desempenho: a máquina virtual e o interpretador de árvore executam os mesmos
//! programas pesados. Ignorada por padrão; rode em release:
//!
//! ```text
//! cargo test --release --test desempenho -- --ignored --nocapture
//! ```
//!
//! Mede só a execução (a tradução para bytecode fica fora do tempo), confere que os dois
//! motores dão a mesma saída e imprime a mediana de algumas repetições. Os números
//! registrados e o método estão em `docs/execucao.md`.

use std::time::{Duration, Instant};

use pinker::checked::CheckedProgram;
use pinker::lexer::tokenize;
use pinker::lower::lower;
use pinker::parser::parse;
use pinker::semantic::check;
use pinker::{bytecode, interpreter, vm};

// pink-nav: start
// pink-resume: Os programas da medição: chamadas recursivas, laço aritmético, laços aninhados, laços longos com u64 e cópia de textos.
// pink-keywords: desempenho, medição, benchmark, programa, recursão, laço, texto
/// O nome e o código-fonte de cada programa medido. Cada um escreve pouco: o tempo é o da
/// computação, não o da saída.
const PROGRAMS: &[(&str, &str)] = &[
    (
        "recursão (fibonacci 27)",
        "fun fib(n: int) -> int
    if n < 2
        give n
    give fib(n - 1) + fib(n - 2)

fun main()
    say(fib(27))
",
    ),
    (
        "laço aritmético (3 milhões)",
        "fun main()
    let mut i = 0
    let mut soma = 0
    while i < 3_000_000
        soma = soma + i * i % 7
        i = i + 1
    say(soma)
",
    ),
    (
        "laços aninhados (primos < 30 mil)",
        "fun primo(n: int) -> bool
    if n < 2
        give false
    let mut d = 2
    while d * d <= n
        if n % d == 0
            give false
        d = d + 1
    give true

fun main()
    let mut n = 0
    let mut achados = 0
    while n < 30_000
        if primo(n)
            achados = achados + 1
        n = n + 1
    say(achados)
",
    ),
    (
        "laços longos com u64 (Collatz até 30 mil)",
        "fun passos(inicio: u64) -> u64
    let mut n = inicio
    let mut contagem: u64 = 0
    while n != 1
        if n % 2 == 0
            n = n / 2
        else
            n = 3 * n + 1
        contagem = contagem + 1
    give contagem

fun main()
    let mut i: u64 = 1
    let mut total: u64 = 0
    while i < 30_000
        total = total + passos(i)
        i = i + 1
    say(total)
",
    ),
    (
        "cópia de textos (1 milhão)",
        "fun main()
    let mut t = \"a\"
    let mut iguais = 0
    let mut i = 0
    while i < 1_000_000
        if i % 2 == 0
            t = \"a\"
        else
            t = \"b\"
        let copia = t
        if copia == \"b\"
            iguais = iguais + 1
        i = i + 1
    say(iguais)
",
    ),
];

const REPETITIONS: usize = 5;
// pink-nav: end

// pink-nav: start
// pink-resume: Executa cada programa nos dois motores, confere a mesma saída e imprime a mediana dos tempos e a razão entre eles.
// pink-keywords: desempenho, medição, tempo, mediana, vm, interpretador, comparação
fn checked(source: &str) -> CheckedProgram {
    check(&parse(&tokenize(source).unwrap()).unwrap()).unwrap()
}

/// A mediana do tempo de `REPETITIONS` execuções, depois de uma de aquecimento, e a saída.
fn measure(mut run: impl FnMut(&mut Vec<u8>)) -> (Duration, Vec<u8>) {
    let mut output = Vec::new();
    run(&mut output);
    let mut times: Vec<Duration> = (0..REPETITIONS)
        .map(|_| {
            let mut discarded = Vec::new();
            let start = Instant::now();
            run(&mut discarded);
            start.elapsed()
        })
        .collect();
    times.sort();
    (times[REPETITIONS / 2], output)
}

#[test]
#[ignore = "medição de desempenho; rode em release com --ignored --nocapture"]
fn vm_e_interpretador() {
    println!(
        "\n{:<42} {:>16} {:>10} {:>16}",
        "programa", "interpretador ms", "VM ms", "VM/interpretador"
    );
    for (name, source) in PROGRAMS {
        let program = checked(source);
        let lowered = lower(&program);
        bytecode::verify(&lowered).unwrap();
        let (tree, by_tree) = measure(|output| interpreter::run(&program, output).unwrap());
        let (machine, by_vm) = measure(|output| vm::run(&lowered, output).unwrap());
        assert_eq!(by_vm, by_tree, "{name}");
        println!(
            "{name:<42} {:>16.1} {:>10.1} {:>16.2}",
            tree.as_secs_f64() * 1000.0,
            machine.as_secs_f64() * 1000.0,
            machine.as_secs_f64() / tree.as_secs_f64()
        );
    }
}
// pink-nav: end
