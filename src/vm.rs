//! Máquina virtual: executa o bytecode.
//!
//! É iterativa: as chamadas da Pinker não usam a pilha do Rust. Cada chamada tem um quadro
//! com uma posição por variável, e todas compartilham uma pilha de valores. As operações
//! sobre valores, as mensagens das falhas e o limite de chamadas vêm do runtime
//! (`runtime`), o mesmo do interpretador de árvore (`interpreter`), que continua como
//! referência: os dois precisam dar a mesma saída e o mesmo diagnóstico para qualquer
//! programa. As regras de
//! execução estão em `docs/execucao.md`.

use std::io::Write;
use std::rc::Rc;

use crate::ast::BinaryOperator;
use crate::bytecode::{BytecodeProgram, Operation, is_ordering};
use crate::checked::Type;
use crate::diagnostic::Diagnostic;
use crate::runtime::{
    MAX_CALL_DEPTH, Value, binary_values, call_limit, convert, negate, output_error, say,
};
use crate::token::Position;

// pink-nav: start
// pink-resume: Ponto de entrada da VM: executa a main a partir do bytecode verificado e descarrega a saída no fim.
// pink-keywords: vm, máquina virtual, run, executar, main, saída, flush, quadro
/// Executa o programa a partir da `main`, escrevendo a saída de `say` em `output`. O
/// bytecode precisa ter passado por `bytecode::verify` ou ter vindo de `lower`. Devolve a
/// falha de execução, se houver; a saída escrita antes dela é preservada.
pub fn run(program: &BytecodeProgram, output: &mut dyn Write) -> Result<(), Diagnostic> {
    let main = &program.functions[program.main];
    let mut machine = Machine::new(program, output);
    machine.execute()?;
    machine
        .output
        .flush()
        .map_err(|e| output_error(e, main.position))
}

struct Machine<'p> {
    program: &'p BytecodeProgram,
    output: &'p mut dyn Write,
    /// Os valores em uso pelas chamadas ativas, da mais antiga à mais recente.
    stack: Vec<Value>,
    /// As chamadas ativas; a última é a que está executando.
    frames: Vec<Frame>,
}

/// Uma chamada ativa: a função, a próxima instrução e uma posição para cada variável,
/// vazia antes do `let` e depois do bloco.
struct Frame {
    function: usize,
    next: usize,
    slots: Vec<Option<Value>>,
}
// pink-nav: end

impl<'p> Machine<'p> {
    /// A máquina pronta para executar a primeira instrução da `main`.
    fn new(program: &'p BytecodeProgram, output: &'p mut dyn Write) -> Self {
        let main = &program.functions[program.main];
        Machine {
            program,
            output,
            stack: Vec::new(),
            frames: vec![Frame {
                function: program.main,
                next: 0,
                slots: vec![None; main.variables.len()],
            }],
        }
    }
}

impl Machine<'_> {
    // pink-nav: start
    // pink-resume: O laço de execução: busca a próxima instrução da chamada ativa e aplica cada operação à pilha, aos quadros e à saída.
    // pink-keywords: vm, laço, instrução, pilha, salto, chamada, give, say, morph, overflow, limite de chamadas
    fn execute(&mut self) -> Result<(), Diagnostic> {
        while self.step()? {}
        Ok(())
    }

    /// Executa a próxima instrução da chamada ativa. Devolve falso quando a `main`
    /// termina.
    fn step(&mut self) -> Result<bool, Diagnostic> {
        let program = self.program;
        let frame = self.frames.last_mut().expect("há uma chamada ativa");
        let function = &program.functions[frame.function];
        let instruction = &function.code[frame.next];
        frame.next += 1;
        let position = instruction.position;
        match &instruction.operation {
            Operation::Integer(value, _) => self.stack.push(Value::Int(*value)),
            Operation::Bool(value) => self.stack.push(Value::Bool(*value)),
            Operation::Text(value) => self.stack.push(Value::Text(Rc::clone(value))),
            Operation::Load(slot) => {
                let value = frame.slots[*slot]
                    .clone()
                    .expect("o verificador garante que a variável está cheia");
                self.stack.push(value);
            }
            Operation::Store(slot) => {
                let value = self.pop();
                self.frames.last_mut().unwrap().slots[*slot] = Some(value);
            }
            Operation::Clear(slot) => frame.slots[*slot] = None,
            Operation::Binary(operator, int) => {
                let ty = if is_ordering(*operator) {
                    Type::Bool
                } else {
                    Type::Int(*int)
                };
                self.binary(*operator, ty, position)?;
            }
            Operation::Equality(operator, _) => self.binary(*operator, Type::Bool, position)?,
            Operation::Concat => self.binary(BinaryOperator::Add, Type::Text, position)?,
            Operation::Negate(int) => {
                let Value::Int(value) = self.pop() else {
                    unreachable!("o verificador garante um inteiro");
                };
                let result = negate(value, *int, position)?;
                self.stack.push(result);
            }
            Operation::Not => {
                let Value::Bool(value) = self.pop() else {
                    unreachable!("o verificador garante um bool");
                };
                self.stack.push(Value::Bool(!value));
            }
            Operation::Jump(target) => frame.next = *target,
            Operation::JumpFalse(target) => {
                let Value::Bool(value) = self.pop() else {
                    unreachable!("o verificador garante um bool");
                };
                if !value {
                    self.frames.last_mut().unwrap().next = *target;
                }
            }
            Operation::Call(callee) => {
                if self.frames.len() >= MAX_CALL_DEPTH {
                    return Err(call_limit(position));
                }
                let callee_function = &program.functions[*callee];
                let mut slots = vec![None; callee_function.variables.len()];
                let first = self.stack.len() - callee_function.parameters.len();
                for (&parameter, argument) in callee_function
                    .parameters
                    .iter()
                    .zip(self.stack.drain(first..))
                {
                    slots[parameter] = Some(argument);
                }
                self.frames.push(Frame {
                    function: *callee,
                    next: 0,
                    slots,
                });
            }
            Operation::Say(_) => {
                let value = self.pop();
                say(self.output, &value, position)?;
            }
            Operation::Morph(_, to) => {
                let Value::Int(value) = self.pop() else {
                    unreachable!("o verificador garante um inteiro");
                };
                let result = convert(value, *to, position)?;
                self.stack.push(result);
            }
            Operation::Pop => {
                self.pop();
            }
            Operation::Give => {
                // O valor devolvido já está na pilha; o quadro, com as variáveis, é
                // descartado.
                self.frames.pop();
                if self.frames.is_empty() {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn pop(&mut self) -> Value {
        self.stack
            .pop()
            .expect("o verificador garante que a pilha tem um valor")
    }

    /// Desempilha os dois operandos (o da direita está no topo) e empilha o resultado.
    fn binary(
        &mut self,
        operator: BinaryOperator,
        ty: Type,
        position: Position,
    ) -> Result<(), Diagnostic> {
        let right = self.pop();
        let left = self.pop();
        let result = binary_values(operator, left, right, ty, position)?;
        self.stack.push(result);
        Ok(())
    }
    // pink-nav: end
}

// pink-nav: start
// pink-resume: Testes internos de memória: um texto dinâmico é liberado no momento em que o bloco termina (fim normal, break, continue), quando a substituição perde a última referência e depois do give, com a main ainda executando.
// pink-keywords: teste, memória, texto dinâmico, concatenação, liberação, weak, clear, break, continue, give, substituição
#[cfg(test)]
mod tests {
    use std::rc::Weak;

    use super::*;
    use crate::{bytecode, lexer, lower, parser, semantic};

    fn lowered(source: &str) -> BytecodeProgram {
        let tokens = lexer::tokenize(source).unwrap();
        let program = lower::lower(&semantic::check(&parser::parse(&tokens).unwrap()).unwrap());
        bytecode::verify(&program).unwrap();
        program
    }

    /// A próxima instrução da chamada ativa.
    fn next(machine: &Machine) -> &'static str {
        let frame = machine.frames.last().unwrap();
        let operation = &machine.program.functions[frame.function].code[frame.next].operation;
        match operation {
            Operation::Text(value) if &**value == "depois" => "depois",
            Operation::Clear(_) => "clear",
            Operation::Say(_) => "say",
            Operation::Give => "give",
            Operation::Store(_) => "store",
            _ => "outra",
        }
    }

    /// Executa até que a próxima instrução seja do tipo dado; falha se a main terminar.
    fn run_until(machine: &mut Machine, wanted: &str) {
        while next(machine) != wanted {
            assert!(
                machine.step().unwrap(),
                "a main terminou antes de '{wanted}'"
            );
        }
    }

    /// Uma referência fraca ao texto da variável da chamada ativa.
    fn text_in(machine: &Machine, slot: usize) -> Weak<str> {
        match &machine.frames.last().unwrap().slots[slot] {
            Some(Value::Text(text)) => Rc::downgrade(text),
            other => panic!("esperava um texto na variável #{slot}: {other:?}"),
        }
    }

    #[test]
    fn texto_dinamico_e_liberado_no_fim_do_bloco() {
        let program = lowered(
            "fun main()\n    let base = \"Pin\"\n    if true\n        let t = base + \"ker\"\n        say(t)\n    say(\"depois\")\n",
        );
        let mut output = Vec::new();
        let mut machine = Machine::new(&program, &mut output);
        run_until(&mut machine, "say");
        let t = text_in(&machine, 1);
        assert_eq!(&*t.upgrade().unwrap(), "Pinker");
        // O bloco do if terminou; a main continua, e o texto já foi liberado.
        run_until(&mut machine, "depois");
        assert!(t.upgrade().is_none());
    }

    #[test]
    fn break_e_continue_liberam_os_textos_dos_blocos_de_que_saem() {
        // t é mutável para que a falta de um clear não seja apanhada pelo verificador
        // (um let imutável guardado de novo na volta seguinte): o teste observa a
        // memória, não o verificador.
        let program = lowered(
            "fun main()\n    let mut i = 0\n    loop\n        i = i + 1\n        let mut t = \"x\" + \"y\"\n        if i == 1\n            continue\n        if true\n            let u = t + \"z\"\n            break\n    say(\"depois\")\n",
        );
        let mut output = Vec::new();
        let mut machine = Machine::new(&program, &mut output);
        // Primeira volta: o continue esvazia t antes de voltar ao início do laço.
        run_until(&mut machine, "clear");
        let first = text_in(&machine, 1);
        assert!(first.upgrade().is_some());
        assert!(machine.step().unwrap());
        assert!(first.upgrade().is_none());
        // Segunda volta: o break sai de dois blocos e esvazia u e t.
        run_until(&mut machine, "clear");
        let (t, u) = (text_in(&machine, 1), text_in(&machine, 2));
        assert_eq!(&*u.upgrade().unwrap(), "xyz");
        run_until(&mut machine, "depois");
        assert!(t.upgrade().is_none());
        assert!(u.upgrade().is_none());
    }

    #[test]
    fn substituicao_solta_so_o_texto_antigo() {
        let program = lowered(
            "fun main()\n    let a = \"p\" + \"q\"\n    let mut b = a\n    b = b + \"!\"\n    say(a)\n    say(b)\n    say(\"depois\")\n",
        );
        let mut output = Vec::new();
        let mut machine = Machine::new(&program, &mut output);
        run_until(&mut machine, "say");
        let a = text_in(&machine, 0);
        let b = text_in(&machine, 1);
        // b foi substituída: o texto de a só tem a própria variável (e o load do say).
        assert_eq!(&*b.upgrade().unwrap(), "pq!");
        assert_eq!(a.strong_count(), 2);
        run_until(&mut machine, "depois");
        assert_eq!(a.strong_count(), 1);
        assert_eq!(b.strong_count(), 1);
        // O fim do corpo da main esvazia as duas antes do give final.
        run_until(&mut machine, "give");
        assert!(a.upgrade().is_none());
        assert!(b.upgrade().is_none());
    }

    #[test]
    fn texto_devolvido_sobrevive_ao_quadro_e_e_liberado_depois() {
        let program = lowered(
            "fun junta(a: text, b: text) -> text\n    if true\n        let r = a + b\n        give r\n    give a\n\nfun main()\n    let x = junta(\"p\", \"q\")\n    say(x)\n    say(\"depois\")\n",
        );
        let mut output = Vec::new();
        let mut machine = Machine::new(&program, &mut output);
        run_until(&mut machine, "give");
        assert_eq!(machine.frames.len(), 2);
        let r = text_in(&machine, 2);
        // O give descarta o quadro de junta: só o valor devolvido segura o texto.
        assert!(machine.step().unwrap());
        assert_eq!(machine.frames.len(), 1);
        assert_eq!(&*r.upgrade().unwrap(), "pq");
        assert_eq!(r.strong_count(), 1);
        run_until(&mut machine, "depois");
        assert_eq!(r.strong_count(), 1);
        run_until(&mut machine, "give");
        assert!(r.upgrade().is_none());
    }
}
// pink-nav: end
