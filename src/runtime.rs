//! Runtime: os serviços que a Pinker oferece enquanto o programa executa.
//!
//! Os valores em execução e as operações sobre eles (aritmética com overflow conferido,
//! divisão e resto, comparações, negação, concatenação de textos, a conversão de `morph`,
//! a escrita de `say`), as falhas definidas de execução com as suas mensagens e o limite
//! de chamadas aninhadas. A máquina virtual (`vm`) e o interpretador de árvore
//! (`interpreter`) usam este mesmo código: cada operação fica escrita num lugar só. Isso
//! não basta para os dois concordarem, porque a ordem de avaliação, o fluxo e a gestão
//! dos quadros são de cada motor; por isso os testes executam os dois e comparam com
//! resultados esperados. As regras estão em `docs/execucao.md`.

use std::fmt;
use std::io::{self, Write};
use std::rc::Rc;

use crate::ast::BinaryOperator;
use crate::checked::{IntType, Type, arithmetic};
use crate::diagnostic::{Diagnostic, error};
use crate::token::Position;

/// Limite desta implementação para chamadas ativas ao mesmo tempo (aninhadas), contando a
/// `main`. Chamadas que já terminaram não contam.
pub const MAX_CALL_DEPTH: usize = 10_000;

// pink-nav: start
// pink-resume: Valores em execução (inteiro, bool, texto compartilhado) e como say os escreve.
// pink-keywords: valor, inteiro, bool, texto, rc, representação, say, display
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Sempre dentro da faixa do tipo estático da expressão que o produziu.
    Int(i128),
    Bool(bool),
    Text(Rc<str>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(value) => write!(f, "{value}"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::Text(value) => f.write_str(value),
        }
    }
}
// pink-nav: end

// pink-nav: start
// pink-resume: Operações sobre valores já avaliados: aritmética verificada, comparações, negação, morph, say e as falhas de execução.
// pink-keywords: operação, valor, aritmética, overflow, divisão por zero, comparação, negação, morph, say, limite de chamadas
/// O tipo inteiro de um tipo que a análise semântica já garantiu ser inteiro.
pub(crate) fn integer_type(ty: Type) -> IntType {
    match ty {
        Type::Int(int) => int,
        other => unreachable!("tipo verificado como inteiro: {other}"),
    }
}

/// Uma operação binária sobre dois valores já avaliados. `and` e `or` não passam por
/// aqui: o curto-circuito decide antes se o operando da direita é avaliado. `ty` é o tipo
/// do resultado.
pub(crate) fn binary_values(
    operator: BinaryOperator,
    left: Value,
    right: Value,
    ty: Type,
    position: Position,
) -> Result<Value, Diagnostic> {
    use BinaryOperator::*;
    match (operator, &left, &right) {
        (Equal, ..) => return Ok(Value::Bool(left == right)),
        (NotEqual, ..) => return Ok(Value::Bool(left != right)),
        (Add, Value::Text(a), Value::Text(b)) => return Ok(Value::Text(concatenate(a, b))),
        _ => {}
    }
    let (Value::Int(a), Value::Int(b)) = (left, right) else {
        unreachable!("operandos verificados como inteiros");
    };
    let result = match operator {
        Less => return Ok(Value::Bool(a < b)),
        LessEqual => return Ok(Value::Bool(a <= b)),
        Greater => return Ok(Value::Bool(a > b)),
        GreaterEqual => return Ok(Value::Bool(a >= b)),
        Divide | Remainder if b == 0 => return Err(error("divisão por zero", position)),
        _ => arithmetic(operator, a, b),
    };
    let int = integer_type(ty);
    let symbol = operator.as_str();
    match result {
        Some(value) if int.contains(value) => Ok(Value::Int(value)),
        Some(value) => Err(error(
            format!("overflow: {a} {symbol} {b} = {value} não cabe em {int}"),
            position,
        )),
        None => Err(error(
            format!("overflow: o resultado de {a} {symbol} {b} não cabe em {int}"),
            position,
        )),
    }
}

/// Um texto novo com o conteúdo de `left` seguido do de `right`, byte a byte, sem
/// normalização. É o único lugar em que a execução cria um texto: o armazenamento é
/// dinâmico, contado, e é liberado quando a última referência é perdida. Uma falha de
/// alocação encerra o processo, como qualquer esgotamento de memória do Rust.
pub(crate) fn concatenate(left: &str, right: &str) -> Rc<str> {
    let mut joined = String::with_capacity(left.len() + right.len());
    joined.push_str(left);
    joined.push_str(right);
    Rc::from(joined)
}

pub(crate) fn negate(value: i128, int: IntType, position: Position) -> Result<Value, Diagnostic> {
    if !int.contains(-value) {
        return Err(error(
            format!("overflow: -({value}) não cabe em {int}"),
            position,
        ));
    }
    Ok(Value::Int(-value))
}

/// A conversão de `morph`: o valor não muda, mas precisa caber no tipo de destino.
pub(crate) fn convert(
    value: i128,
    target: IntType,
    position: Position,
) -> Result<Value, Diagnostic> {
    if !target.contains(value) {
        return Err(error(
            format!("conversão fora da faixa: {value} não cabe em {target}"),
            position,
        ));
    }
    Ok(Value::Int(value))
}

pub(crate) fn say(
    output: &mut dyn Write,
    value: &Value,
    position: Position,
) -> Result<(), Diagnostic> {
    writeln!(output, "{value}").map_err(|e| output_error(e, position))
}

pub(crate) fn output_error(e: io::Error, position: Position) -> Diagnostic {
    error(format!("não foi possível escrever a saída: {e}"), position)
}

pub(crate) fn call_limit(position: Position) -> Diagnostic {
    error(
        format!("limite de {MAX_CALL_DEPTH} chamadas aninhadas excedido"),
        position,
    )
}
// pink-nav: end
