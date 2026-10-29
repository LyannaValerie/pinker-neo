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
    let mut machine = Machine {
        program,
        output,
        stack: Vec::new(),
        frames: vec![Frame {
            function: program.main,
            next: 0,
            slots: vec![None; main.variables.len()],
        }],
    };
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

impl Machine<'_> {
    // pink-nav: start
    // pink-resume: O laço de execução: busca a próxima instrução da chamada ativa e aplica cada operação à pilha, aos quadros e à saída.
    // pink-keywords: vm, laço, instrução, pilha, salto, chamada, give, say, morph, overflow, limite de chamadas
    fn execute(&mut self) -> Result<(), Diagnostic> {
        let program = self.program;
        loop {
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
                        return Ok(());
                    }
                }
            }
        }
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
