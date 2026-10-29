# Análise semântica da primeira versão

As regras que a análise semântica (`src/semantic.rs`) verifica depois do parser. O
resultado é a árvore verificada (`src/checked.rs`): o programa com os tipos resolvidos,
cada uso de variável ligado à sua declaração e cada chamada ligada à função chamada. É a
entrada da tradução para bytecode (`src/lower.rs`). No `.s`, a árvore vem acompanhada da tabela de símbolos: as
funções com as suas assinaturas e as variáveis de cada função, com tipo, posição e
espécie (ver [`arquivos.md`](arquivos.md)). A análise para no primeiro erro.

## Etapas

| Etapa | Obrigação |
|---|---|
| Coletar assinaturas | Registrar todas as funções antes de verificar os corpos. Isso permite chamar uma função declarada depois e a recursão. Funções duplicadas são erro. |
| Resolver tipos | Reconhecer `int`, `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `bool` e `text`. `int` vira `i64`. Um tipo desconhecido é erro. |
| Validar o executável | Exigir exatamente uma `main`, sem parâmetros e sem resultado. |
| Resolver nomes | Ligar cada uso a um parâmetro, variável ou função, conferindo escopos e uso antes da declaração. |
| Verificar expressões | Operadores, argumentos, resultados, condições e o tipo dos literais. |
| Verificar instruções | Mutabilidade, atribuições, `give`, `break` e `continue`. |
| Verificar o fluxo | Rejeitar uma função com resultado que possa chegar ao fim sem `give`. |

## Escopos

- Cada bloco abre um escopo. Uma variável deixa de existir no fim do bloco em que foi
  declarada.
- Declarar duas vezes o mesmo nome no mesmo escopo é erro. Num bloco interno, um nome
  pode sombrear outro de um escopo externo.
- Os parâmetros e o nível de cima do corpo da função dividem o mesmo escopo: redeclarar
  um parâmetro ali é erro.
- O valor de um `let` é verificado antes da declaração: em `let x = x + 1`, o `x` da
  direita precisa existir num escopo externo.
- Os parâmetros são imutáveis.
- Funções e variáveis vivem em espaços de nomes separados. Uma chamada `f(...)` sempre
  procura uma função; um nome solto sempre procura uma variável. Funções ainda não são
  valores.

## Nomes predefinidos

`say` e `morph` não são palavras reservadas, mas são funções predefinidas e não podem ser
redeclaradas como função, variável ou parâmetro.

## Inteiros

- Não há promoção implícita entre inteiros de tipos diferentes: `a + b` com `a: i32` e
  `b: i64` é erro. Converter um valor exige `morph`.
- Um literal inteiro recebe o tipo do contexto: a anotação do `let`, a variável da
  atribuição, o parâmetro da chamada, o resultado da função ou o outro operando (em
  `x + 1` com `x: u8`, o `1` é `u8`). Sem contexto, é `int` (`i64`).
- Um literal fora da faixa do seu tipo é erro: `let a: u8 = 256`.
- `-` aplicado direto a um literal forma um literal negativo, o que permite escrever o
  mínimo de `i64`, `-9223372036854775808`.

## Operadores

| Operadores | Operandos | Resultado |
|---|---|---|
| `+` `-` `*` `/` `%` | dois inteiros do mesmo tipo | o mesmo tipo |
| `+` | dois `text` | `text` (concatenação) |
| `<` `<=` `>` `>=` | dois inteiros do mesmo tipo | `bool` |
| `==` `!=` | dois valores do mesmo tipo (inteiro, `bool` ou `text`) | `bool` |
| `and` `or` | dois `bool` | `bool` |
| `not` | um `bool` | `bool` |
| `-` prefixo | um inteiro com sinal | o mesmo tipo |

`if` e `while` exigem uma condição `bool`.

O `+` mantém a precedência e a associatividade de sempre: `"a" + "b" + "c"` é
`("a" + "b") + "c"`, e `"a" + "b" == "ab"` compara o resultado da concatenação. Não há
conversão implícita: `"a" + 1`, `1 + "a"` e qualquer `+` com `bool` são erros. Os
outros operadores aritméticos não aceitam textos.

## Avaliação constante

Uma expressão que só depende de literais é avaliada durante a análise:

- Overflow é erro de compilação: `let x: u8 = 200 + 100`.
- Divisão ou resto por um divisor constante zero é erro de compilação, mesmo que o
  dividendo seja uma variável: `x / 0`.
- O mínimo de um inteiro com sinal dividido por `-1` é overflow; o resto é zero.
- **Código que comprovadamente não executa não gera falha de avaliação.** Há três casos:
  o operando que o curto-circuito pula (`e` em `false and e` e em `true or e`), o ramo de
  um `if` cuja condição é constante (o bloco de `if false`, o `else` de `if true`) e o
  corpo de `while false`. Nesses casos, uma divisão por zero ou um overflow constante não
  é erro: `false and 1 / 0 == 1` e `if false` seguido de `let value = 1 / 0` são aceitos.
- Mesmo assim, esse código é verificado por inteiro: nomes, tipos e a faixa dos literais.
  `if false` seguido de `let value: int = "texto"` continua sendo erro.
- **Limitação:** o código que vem depois de `give`, `break` ou `continue` no mesmo bloco
  também nunca executa, mas a análise ainda não o reconhece. Uma falha de avaliação
  constante nele continua sendo erro.
- **Textos não são constantes.** A avaliação constante só conhece inteiros e lógicos. A
  concatenação de dois literais (`"a" + "b"`) não é calculada nem dobrada: fica na árvore
  e executa como qualquer outra. Ela não tem falha que se possa apontar antes da
  execução (só a falta de memória, que é fatal). Uma condição que depende de textos, como
  `"a" + "b" == "ab"`, também não é constante: o código dentro dela é avaliado
  normalmente.

## Conversões com `morph`

- `morph` converte entre quaisquer dois tipos inteiros, inclusive o mesmo tipo.
  `bool` e `text` não se convertem.
- O tipo de destino vem do contexto: `let b: i64 = morph(a)`. Sem destino, é erro; a
  análise não adivinha.
- Uma conversão que sempre cabe (de `i32` para `i64`, por exemplo) sempre funciona.
- Uma conversão que pode não caber (de `i64` para `u8`, ou entre com e sem sinal) é
  conferida: com um argumento constante fora da faixa, é erro de compilação; com um valor
  conhecido só na execução, é uma falha definida em tempo de execução. Nunca trunca em
  silêncio.

## Chamadas

- Os argumentos precisam ter os tipos dos parâmetros, na mesma quantidade.
- Uma função sem resultado não pode ser usada como valor. Numa chamada solta, o resultado
  de uma função com resultado é descartado.
- `say` recebe exatamente um argumento, de qualquer tipo.

## Instruções e fluxo

- Só uma variável `let mut` pode receber atribuição, e o valor precisa ter o tipo dela.
- `give valor` exige uma função com resultado e um valor desse tipo. `give` sem valor
  exige uma função sem resultado.
- `break` e `continue` só valem dentro de `while` ou `loop`.
- Uma função com `-> T` não pode alcançar o fim do corpo sem fornecer o resultado. O que
  se verifica é que o fim nunca é alcançado, e não que a função sempre devolve: um `loop`
  que só tem `continue` nunca devolve, mas também nunca chega ao fim, e é aceito.
- Um bloco nunca chega ao fim quando tem um `give`, um `if` com `else` em que nenhum dos
  dois lados chega ao fim, ou um `loop` sem `break` que saia dele. Cada `break` pertence
  ao laço mais interno que o contém: em `loop` com outro `loop` dentro, um `break` no
  interno não faz o externo terminar.
- Um `while` sempre conta como podendo chegar ao fim, mesmo com condição `true`. É uma
  análise conservadora desta versão.

```text
fun wait_forever() -> int
    loop
        loop
            break    # encerra só o laço interno; a função nunca chega ao fim
```

## `--emit=semantic`

`pinker <arquivo.pk> --emit=semantic` grava a árvore verificada em `<arquivo>.s`. O
formato é o de `--emit=ast`, com o tipo de cada expressão. Cada variável aparece como
`nome#n`, em que `n` identifica a declaração dentro da função; assim o sombreamento fica
visível. A declaração (`Let`) traz também a posição do nome. A etapa não executa o
programa; o arquivo `.s` serve de entrada para a tradução para bytecode (ver
[`arquivos.md`](arquivos.md)).

```text
5:5          Let n#0@5:9: u8
...
7:9              Let n#1@7:13: i64
7:17               Inteiro -1: i64
8:9              Chamada say (predefinida)
8:13               Variável n#1: i64
```

`--emit=tokens` e `--emit=ast` continuam parando nas suas etapas, mesmo quando a análise
semântica rejeitaria o programa.
