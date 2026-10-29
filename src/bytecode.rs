//! Bytecode: as instruções da máquina virtual da Pinker, o arquivo `.b` e o verificador.
//!
//! Cada função vira uma lista de instruções de pilha, traduzida da árvore verificada por
//! `lower`. As instruções levam os tipos (`add.u8`, `eq.text`), e cada uma guarda a posição
//! do código-fonte de onde veio, para os diagnósticos. O formato do `.b` está em
//! `docs/arquivos.md`.

use std::rc::Rc;

use crate::artifact::{
    Format, FormatError, format_error, header, literal, parse_position, split_field,
};
use crate::ast::BinaryOperator;
use crate::checked::{
    FunctionSymbols, IntType, Type, Variable, parse_type, read_symbols, render_symbols, sections,
    variable_reference,
};
use crate::diagnostic::{Diagnostic, error};
use crate::token::{Position, TokenKind, quote_text};

// pink-nav: start
// pink-resume: O programa em bytecode: funções com a tabela de variáveis e as instruções de pilha, cada uma com a sua posição.
// pink-keywords: bytecode, instrução, operação, pilha, função, programa, salto, constante
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeProgram {
    pub functions: Vec<BytecodeFunction>,
    /// Índice da função `main` em `functions`.
    pub main: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeFunction {
    pub name: String,
    /// Índices dos parâmetros em `variables`, na ordem da assinatura.
    pub parameters: Vec<usize>,
    pub result: Option<Type>,
    /// Todas as variáveis da função; cada uma é uma posição no quadro da chamada.
    pub variables: Vec<Variable>,
    pub code: Vec<Instruction>,
    /// Posição do `fun`, onde a função começa.
    pub position: Position,
    /// Posição do nome, que a tabela de símbolos registra.
    pub name_position: Position,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub operation: Operation,
    /// A posição do código-fonte a que a instrução corresponde; uma falha aponta para ela.
    pub position: Position,
}

/// Uma instrução da máquina de pilha. "Empilha" e "desempilha" se referem à pilha de
/// valores da chamada em andamento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Empilha uma constante. O inteiro já está dentro da faixa do tipo.
    Integer(i128, IntType),
    Bool(bool),
    Text(Rc<str>),
    /// Empilha o valor da variável.
    Load(usize),
    /// Desempilha um valor e o guarda na variável.
    Store(usize),
    /// Esvazia a variável: o bloco em que ela foi declarada terminou.
    Clear(usize),
    /// Desempilha dois inteiros do tipo e empilha o resultado: aritmética (`+ - * / %`),
    /// com o resultado no mesmo tipo, ou comparação de ordem (`< <= > >=`), com `bool`.
    Binary(BinaryOperator, IntType),
    /// Desempilha dois valores do tipo e empilha `bool`: `==` ou `!=`.
    Equality(BinaryOperator, Type),
    /// Troca o sinal de um inteiro com sinal.
    Negate(IntType),
    Not,
    /// Continua na instrução de número dado.
    Jump(usize),
    /// Desempilha um `bool`; se for falso, continua na instrução de número dado.
    JumpFalse(usize),
    /// Desempilha os argumentos e chama a função; o resultado, se houver, é empilhado
    /// quando ela devolve.
    Call(usize),
    /// Desempilha um valor do tipo e o escreve com uma quebra de linha.
    Say(Type),
    /// Desempilha um inteiro do primeiro tipo e o empilha no segundo, se couber.
    Morph(IntType, IntType),
    /// Descarta o valor do topo: o resultado de uma chamada solta.
    Pop,
    /// Encerra a chamada; numa função com resultado, desempilha o valor devolvido.
    Give,
}

/// O nome de cada operação binária no bytecode.
const MNEMONICS: &[(BinaryOperator, &str)] = &[
    (BinaryOperator::Add, "add"),
    (BinaryOperator::Subtract, "sub"),
    (BinaryOperator::Multiply, "mul"),
    (BinaryOperator::Divide, "div"),
    (BinaryOperator::Remainder, "rem"),
    (BinaryOperator::Less, "lt"),
    (BinaryOperator::LessEqual, "le"),
    (BinaryOperator::Greater, "gt"),
    (BinaryOperator::GreaterEqual, "ge"),
    (BinaryOperator::Equal, "eq"),
    (BinaryOperator::NotEqual, "ne"),
];

fn mnemonic(operator: BinaryOperator) -> &'static str {
    MNEMONICS
        .iter()
        .find(|(candidate, _)| *candidate == operator)
        .map(|(_, name)| *name)
        .expect("and e or não são instruções")
}

fn is_equality(operator: BinaryOperator) -> bool {
    matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
}

pub(crate) fn is_ordering(operator: BinaryOperator) -> bool {
    use BinaryOperator::*;
    matches!(operator, Less | LessEqual | Greater | GreaterEqual)
}

fn is_arithmetic(operator: BinaryOperator) -> bool {
    use BinaryOperator::*;
    matches!(operator, Add | Subtract | Multiply | Divide | Remainder)
}
// pink-nav: end

// pink-nav: start
// pink-resume: Grava o bytecode no formato .b (saída de --emit=bytecode): a tabela de símbolos e as instruções numeradas de cada função.
// pink-keywords: emit, render, gravação, arquivo .b, bytecode, instrução, mnemônico
/// O arquivo `.b`: a mesma tabela de símbolos do `.s` e uma seção `[código <função>
/// <posição>]` por função, com as instruções numeradas a partir de 0. Os saltos apontam
/// para o número da instrução.
pub fn render(program: &BytecodeProgram, source: &str) -> String {
    let mut output = header(Format::Bytecode, source);
    let symbols: Vec<FunctionSymbols> = program
        .functions
        .iter()
        .map(|function| FunctionSymbols {
            name: &function.name,
            name_position: function.name_position,
            parameters: &function.parameters,
            result: function.result,
            variables: &function.variables,
        })
        .collect();
    render_symbols(&mut output, &symbols);
    for function in &program.functions {
        let position = |p: Position| format!("{}:{}", p.line, p.column);
        let number_width = function
            .code
            .len()
            .saturating_sub(1)
            .to_string()
            .len()
            .max(2);
        let position_width = function
            .code
            .iter()
            .map(|instruction| position(instruction.position).len())
            .fold("POSIÇÃO".chars().count(), usize::max);
        output.push_str(&format!(
            "\n[código {} {}]\n",
            function.name,
            position(function.position)
        ));
        output.push_str(&format!(
            "{:<number_width$}  {:<position_width$}  INSTRUÇÃO\n",
            "#N", "POSIÇÃO"
        ));
        for (index, instruction) in function.code.iter().enumerate() {
            output.push_str(&format!(
                "{index:<number_width$}  {:<position_width$}  {}\n",
                position(instruction.position),
                operation_text(&instruction.operation, function, program)
            ));
        }
    }
    output
}

fn operation_text(
    operation: &Operation,
    function: &BytecodeFunction,
    program: &BytecodeProgram,
) -> String {
    let variable = |index: usize| format!("{}#{index}", function.variables[index].name);
    match operation {
        Operation::Integer(value, int) => format!("const.{int} {value}"),
        Operation::Bool(value) => format!("const.bool {value}"),
        Operation::Text(value) => format!("const.text {}", quote_text(value)),
        Operation::Load(index) => format!("load {}", variable(*index)),
        Operation::Store(index) => format!("store {}", variable(*index)),
        Operation::Clear(index) => format!("clear {}", variable(*index)),
        Operation::Binary(operator, int) => format!("{}.{int}", mnemonic(*operator)),
        Operation::Equality(operator, ty) => format!("{}.{ty}", mnemonic(*operator)),
        Operation::Negate(int) => format!("neg.{int}"),
        Operation::Not => "not".to_string(),
        Operation::Jump(target) => format!("jump {target}"),
        Operation::JumpFalse(target) => format!("jump_false {target}"),
        Operation::Call(index) => format!("call {}", program.functions[*index].name),
        Operation::Say(ty) => format!("say.{ty}"),
        Operation::Morph(from, to) => format!("morph.{from}.{to}"),
        Operation::Pop => "pop".to_string(),
        Operation::Give => "give".to_string(),
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Lê um arquivo .b de volta: a tabela de símbolos, uma seção de código por função e cada instrução com os nomes conferidos.
// pink-keywords: read, leitura, arquivo .b, bytecode, seção, instrução, mnemônico, erro de formato
/// Lê um arquivo `.b` (sem conferir o cabeçalho, que `artifact::detect` já leu). Confere a
/// forma, as seções e os nomes; a coerência da pilha, dos tipos e dos saltos fica para
/// `verify`, que precisa rodar antes da máquina virtual.
pub fn read(content: &str) -> Result<BytecodeProgram, FormatError> {
    let sections = sections(content, "código", "[código <função> <linha:coluna>]")?;
    let symbols = read_symbols(&sections)?;
    if let Some((line, ..)) = sections.body.get(symbols.functions.len()) {
        return Err(format_error(
            *line,
            "seção de código a mais: há uma seção [código] por função da tabela",
        ));
    }
    if let Some(symbol) = symbols.functions.get(sections.body.len()) {
        let last = content.lines().count().max(1);
        return Err(format_error(
            last,
            format!("falta a seção [código {}]", symbol.name),
        ));
    }

    let mut functions = Vec::new();
    for ((symbol, table), (line, title, rows)) in symbols
        .functions
        .iter()
        .zip(symbols.tables)
        .zip(&sections.body)
    {
        let invalid = || {
            format_error(
                *line,
                format!(
                    "esperava a seção [código {} <linha:coluna>], na ordem da tabela de funções",
                    symbol.name
                ),
            )
        };
        let (name, position) = split_field(title.ok_or_else(invalid)?);
        if name != symbol.name {
            return Err(invalid());
        }
        let position = parse_position(position, *line)?;

        let variables: Vec<Variable> = table.into_iter().map(|(_, variable)| variable).collect();
        let parameters: Vec<usize> = (0..variables.len())
            .filter(|&index| variables[index].parameter)
            .collect();
        let types: Vec<Type> = parameters.iter().map(|&i| variables[i].ty).collect();
        if types != symbol.parameters {
            return Err(format_error(
                symbol.line,
                format!(
                    "a assinatura de '{}' diz ({}), mas os parâmetros da tabela de variáveis são ({})",
                    symbol.name,
                    type_list(&symbol.parameters),
                    type_list(&types)
                ),
            ));
        }

        let mut code = Vec::new();
        for (index, &(line, text)) in rows.iter().enumerate() {
            let (number, rest) = split_field(text);
            if number != index.to_string() {
                return Err(format_error(
                    line,
                    format!("esperava a instrução {index}, encontrei '{number}'"),
                ));
            }
            let (position, operation) = split_field(rest);
            let position = parse_position(position, line)?;
            let operation =
                read_operation(operation.trim_end(), line, &variables, &symbols.by_name)?;
            code.push(Instruction {
                operation,
                position,
            });
        }
        functions.push(BytecodeFunction {
            name: symbol.name.clone(),
            parameters,
            result: symbol.result,
            variables,
            code,
            position,
            name_position: symbol.position,
        });
    }
    let main = *symbols
        .by_name
        .get("main")
        .ok_or_else(|| format_error(1, "o programa não tem uma função 'main'"))?;
    Ok(BytecodeProgram { functions, main })
}

fn type_list(types: &[Type]) -> String {
    let names: Vec<String> = types.iter().map(Type::to_string).collect();
    names.join(", ")
}

/// Uma instrução: o mnemônico, com os tipos depois de pontos (`add.i64`, `morph.u8.i64`),
/// e o operando, se houver.
fn read_operation(
    text: &str,
    line: usize,
    variables: &[Variable],
    functions: &std::collections::HashMap<String, usize>,
) -> Result<Operation, FormatError> {
    let (word, operand) = split_field(text);
    let mut parts = word.split('.');
    let name = parts.next().unwrap_or_default();
    let types: Vec<&str> = parts.collect();
    let invalid = |message: &str| format_error(line, format!("{message} em '{text}'"));

    let expected_types = match name {
        "const" | "add" | "sub" | "mul" | "div" | "rem" | "lt" | "le" | "gt" | "ge" | "eq"
        | "ne" | "neg" | "say" => 1,
        "morph" => 2,
        "load" | "store" | "clear" | "not" | "jump" | "jump_false" | "call" | "pop" | "give" => 0,
        _ => {
            return Err(format_error(
                line,
                format!("instrução desconhecida '{word}'"),
            ));
        }
    };
    if types.len() != expected_types {
        return Err(invalid(&format!(
            "'{name}' leva {expected_types} tipo(s) depois de ponto"
        )));
    }
    let has_operand = matches!(
        name,
        "const" | "load" | "store" | "clear" | "jump" | "jump_false" | "call"
    );
    if has_operand == operand.is_empty() {
        return Err(invalid(if has_operand {
            "falta o operando"
        } else {
            "operando inesperado"
        }));
    }
    let ty = |index: usize| parse_type(types[index], line);
    let int = |index: usize| match ty(index)? {
        Type::Int(int) => Ok(int),
        _ => Err(invalid("esperava um tipo inteiro")),
    };
    let variable = |text: &str| -> Result<usize, FormatError> {
        let (name, index) = variable_reference(text, line)?;
        match variables.get(index) {
            Some(declared) if declared.name == name => Ok(index),
            Some(declared) => Err(format_error(
                line,
                format!(
                    "a variável #{index} se chama '{}', não '{name}'",
                    declared.name
                ),
            )),
            None => Err(format_error(
                line,
                format!("a variável #{index} não está na tabela"),
            )),
        }
    };
    let target = |text: &str| -> Result<usize, FormatError> {
        text.parse()
            .map_err(|_| invalid("esperava o número de uma instrução"))
    };

    let operation = match name {
        "const" => match ty(0)? {
            Type::Int(int) => {
                let digits = operand.strip_prefix('-').unwrap_or(operand);
                if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(invalid("esperava um inteiro decimal"));
                }
                let value = operand
                    .parse()
                    .map_err(|_| invalid("inteiro grande demais"))?;
                Operation::Integer(value, int)
            }
            Type::Bool => match operand {
                "true" => Operation::Bool(true),
                "false" => Operation::Bool(false),
                _ => return Err(invalid("esperava true ou false")),
            },
            Type::Text => match literal(operand, line)? {
                TokenKind::Text(value) => Operation::Text(Rc::from(value.as_str())),
                _ => return Err(invalid("esperava um texto entre aspas")),
            },
        },
        "load" => Operation::Load(variable(operand)?),
        "store" => Operation::Store(variable(operand)?),
        "clear" => Operation::Clear(variable(operand)?),
        "eq" | "ne" => {
            let operator = if name == "eq" {
                BinaryOperator::Equal
            } else {
                BinaryOperator::NotEqual
            };
            Operation::Equality(operator, ty(0)?)
        }
        "neg" => Operation::Negate(int(0)?),
        "not" => Operation::Not,
        "jump" => Operation::Jump(target(operand)?),
        "jump_false" => Operation::JumpFalse(target(operand)?),
        "call" => Operation::Call(
            *functions
                .get(operand)
                .ok_or_else(|| format_error(line, format!("a função '{operand}' não existe")))?,
        ),
        "say" => Operation::Say(ty(0)?),
        "morph" => Operation::Morph(int(0)?, int(1)?),
        "pop" => Operation::Pop,
        "give" => Operation::Give,
        _ => {
            let operator = MNEMONICS
                .iter()
                .find(|(_, candidate)| *candidate == name)
                .map(|(operator, _)| *operator)
                .expect("o mnemônico foi reconhecido acima");
            Operation::Binary(operator, int(0)?)
        }
    };
    Ok(operation)
}
// pink-nav: end

// pink-nav: start
// pink-resume: Verificador do bytecode: interpretação abstrata de cada função, com os tipos da pilha e o estado das variáveis em cada instrução alcançável.
// pink-keywords: verify, verificador, bytecode inconsistente, pilha, tipo, variável vazia, imutável, salto, fim do código, interpretação abstrata
/// Confere o bytecode antes da execução. Para cada instrução alcançável, calcula os tipos
/// na pilha e se cada variável está cheia, vazia ou pode estar das duas formas, e exige
/// que todos os caminhos que chegam a ela concordem nos tipos da pilha. Recusa uma
/// instrução que desempilhe o tipo errado, leia uma variável que pode estar vazia, guarde
/// de novo numa variável imutável que pode ter valor, salte para fora do código ou deixe a
/// execução passar da última instrução. O bytecode não guarda os blocos: um `clear`
/// encerra a vida de uma variável, e um `store` depois dele começa outra, como o `let` de
/// um laço a cada volta.
pub fn verify(program: &BytecodeProgram) -> Result<(), Diagnostic> {
    let main = program
        .functions
        .get(program.main)
        .filter(|main| main.name == "main")
        .ok_or_else(|| inconsistent("não há uma função 'main'", Position { line: 1, column: 1 }))?;
    if !main.parameters.is_empty() || main.result.is_some() {
        return Err(inconsistent(
            "'main' não pode ter parâmetros nem resultado",
            main.position,
        ));
    }
    for function in &program.functions {
        verify_function(program, function)?;
    }
    Ok(())
}

fn inconsistent(message: impl Into<String>, position: Position) -> Diagnostic {
    error(
        format!("bytecode inconsistente: {}", message.into()),
        position,
    )
}

/// O estado de uma variável num ponto do código, considerando todos os caminhos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Empty,
    Full,
    /// Cheia por um caminho e vazia por outro.
    Maybe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct State {
    stack: Vec<Type>,
    slots: Vec<Slot>,
}

fn stack_text(stack: &[Type]) -> String {
    format!("[{}]", type_list(stack))
}

fn verify_function(
    program: &BytecodeProgram,
    function: &BytecodeFunction,
) -> Result<(), Diagnostic> {
    let mut slots = vec![Slot::Empty; function.variables.len()];
    for &parameter in &function.parameters {
        let variable = function.variables.get(parameter).filter(|v| v.parameter);
        let Some(variable) = variable else {
            return Err(inconsistent("parâmetro sem declaração", function.position));
        };
        if slots[parameter] == Slot::Full || variable.mutable {
            return Err(inconsistent(
                "parâmetro repetido ou mutável",
                variable.position,
            ));
        }
        slots[parameter] = Slot::Full;
    }
    if function.code.is_empty() {
        return Err(inconsistent(
            format!("a função '{}' não tem código", function.name),
            function.position,
        ));
    }

    let mut states: Vec<Option<State>> = vec![None; function.code.len()];
    states[0] = Some(State {
        stack: Vec::new(),
        slots,
    });
    let mut pending = vec![0];
    while let Some(index) = pending.pop() {
        let mut state = states[index].clone().expect("só entra na fila com estado");
        let instruction = &function.code[index];
        let next = step(program, function, index, instruction, &mut state)?;
        for target in next {
            let Some(arrived) = states.get_mut(target) else {
                return Err(inconsistent(
                    if target == function.code.len() {
                        "a execução pode passar da última instrução".to_string()
                    } else {
                        format!("salto para a instrução {target}, fora do código")
                    },
                    instruction.position,
                ));
            };
            let changed = match arrived {
                None => {
                    *arrived = Some(state.clone());
                    true
                }
                Some(previous) => {
                    if previous.stack != state.stack {
                        return Err(inconsistent(
                            format!(
                                "a instrução {target} é alcançada com pilhas diferentes: {} e {}",
                                stack_text(&previous.stack),
                                stack_text(&state.stack)
                            ),
                            function.code[target].position,
                        ));
                    }
                    let mut changed = false;
                    for (old, new) in previous.slots.iter_mut().zip(&state.slots) {
                        if *old != *new && *old != Slot::Maybe {
                            *old = Slot::Maybe;
                            changed = true;
                        }
                    }
                    changed
                }
            };
            if changed {
                pending.push(target);
            }
        }
    }
    Ok(())
}

/// Aplica a instrução ao estado e devolve as instruções seguintes possíveis.
fn step(
    program: &BytecodeProgram,
    function: &BytecodeFunction,
    index: usize,
    instruction: &Instruction,
    state: &mut State,
) -> Result<Vec<usize>, Diagnostic> {
    let position = instruction.position;
    let fail = |message: String| inconsistent(format!("instrução {index}: {message}"), position);
    let pop = |state: &mut State, expected: Option<Type>| -> Result<Type, Diagnostic> {
        let found = state
            .stack
            .pop()
            .ok_or_else(|| fail("a pilha está vazia".to_string()))?;
        match expected {
            Some(expected) if expected != found => Err(fail(format!(
                "esperava {expected} na pilha, encontrei {found}"
            ))),
            _ => Ok(found),
        }
    };
    let variable = |index: usize| {
        function
            .variables
            .get(index)
            .ok_or_else(|| fail(format!("a variável #{index} não existe")))
    };
    let following = vec![index + 1];

    match &instruction.operation {
        Operation::Integer(value, int) => {
            if !int.contains(*value) {
                return Err(fail(format!("o valor {value} não cabe em {int}")));
            }
            state.stack.push(Type::Int(*int));
        }
        Operation::Bool(_) => state.stack.push(Type::Bool),
        Operation::Text(_) => state.stack.push(Type::Text),
        Operation::Load(slot) => {
            let ty = variable(*slot)?.ty;
            if state.slots[*slot] != Slot::Full {
                return Err(fail(format!("a variável #{slot} pode estar vazia aqui")));
            }
            state.stack.push(ty);
        }
        Operation::Store(slot) => {
            let declared = variable(*slot)?;
            pop(state, Some(declared.ty))?;
            if !declared.mutable && state.slots[*slot] != Slot::Empty {
                return Err(fail(format!(
                    "a variável imutável #{slot} pode já ter um valor aqui"
                )));
            }
            state.slots[*slot] = Slot::Full;
        }
        Operation::Clear(slot) => {
            variable(*slot)?;
            state.slots[*slot] = Slot::Empty;
        }
        Operation::Binary(operator, int) => {
            let ty = Type::Int(*int);
            pop(state, Some(ty))?;
            pop(state, Some(ty))?;
            if is_arithmetic(*operator) {
                state.stack.push(ty);
            } else if is_ordering(*operator) {
                state.stack.push(Type::Bool);
            } else {
                return Err(fail(format!(
                    "'{}' não é uma operação entre inteiros",
                    operator.as_str()
                )));
            }
        }
        Operation::Equality(operator, ty) => {
            if !is_equality(*operator) {
                return Err(fail(format!("'{}' não é uma igualdade", operator.as_str())));
            }
            pop(state, Some(*ty))?;
            pop(state, Some(*ty))?;
            state.stack.push(Type::Bool);
        }
        Operation::Negate(int) => {
            if !int.signed() {
                return Err(fail(format!("neg exige um inteiro com sinal, não {int}")));
            }
            pop(state, Some(Type::Int(*int)))?;
            state.stack.push(Type::Int(*int));
        }
        Operation::Not => {
            pop(state, Some(Type::Bool))?;
            state.stack.push(Type::Bool);
        }
        Operation::Jump(target) => return Ok(vec![*target]),
        Operation::JumpFalse(target) => {
            pop(state, Some(Type::Bool))?;
            return Ok(vec![*target, index + 1]);
        }
        Operation::Call(callee) => {
            let callee = program
                .functions
                .get(*callee)
                .ok_or_else(|| fail("chamada a uma função inexistente".to_string()))?;
            for &parameter in callee.parameters.iter().rev() {
                let ty = callee
                    .variables
                    .get(parameter)
                    .ok_or_else(|| fail(format!("'{}' tem um parâmetro inexistente", callee.name)))?
                    .ty;
                pop(state, Some(ty))?;
            }
            if let Some(result) = callee.result {
                state.stack.push(result);
            }
        }
        Operation::Say(ty) => {
            pop(state, Some(*ty))?;
        }
        Operation::Morph(from, to) => {
            pop(state, Some(Type::Int(*from)))?;
            state.stack.push(Type::Int(*to));
        }
        Operation::Pop => {
            pop(state, None)?;
        }
        Operation::Give => {
            let expected: Vec<Type> = function.result.into_iter().collect();
            if state.stack != expected {
                return Err(fail(format!(
                    "give com a pilha {}; esperava {}",
                    stack_text(&state.stack),
                    stack_text(&expected)
                )));
            }
            return Ok(Vec::new());
        }
    }
    Ok(following)
}
// pink-nav: end
