# Gramática da primeira versão

A gramática do subconjunto inicial da Pinker, que o parser (`src/parser.rs`) implementa.
Cada regra corresponde a uma função do parser.

Os nomes em maiúsculas são tokens do lexer: `NOME` (identificador), `INTEIRO`, `TEXTO`,
`QUEBRA` (fim de uma linha lógica), `INDENT`, `DEDENT` e `FIM`. Os demais aparecem entre
aspas, como no código.

## Notação

As regras usam uma notação própria, que não é a sintaxe da Pinker:

| Notação | Significado |
|---|---|
| `soma = ...` | Define a regra `soma` |
| Nome em minúsculas: `soma`, `bloco` | Não terminal: outra regra |
| Nome em maiúsculas (`NOME`, `QUEBRA`) ou texto entre aspas (`"fun"`, `"("`) | Terminal: um token |
| `a \| b` | Alternativa: `a` ou `b` |
| `[ a ]` | Opcional: zero ou uma vez |
| `{ a }` | Repetição: zero ou mais vezes |
| `( a \| b )` | Agrupa partes da própria regra |

- Sem aspas, `=`, `|`, `[ ]`, `{ }` e `( )` são da notação. Um caractere da Pinker
  aparece entre aspas: em `"(" expressão ")"`, os parênteses são os do código; em
  `( "+" | "-" )`, só agrupam as duas alternativas.
- A gramática recebe tokens, não caracteres. Espaços, comentários e recuo já foram
  resolvidos pelo lexer, que entrega `QUEBRA` no fim de cada linha lógica e `INDENT` e
  `DEDENT` quando o recuo aumenta ou volta. Por isso `bloco` fala de `INDENT` e `DEDENT`,
  e não de espaços.
- Seguir a gramática só prova a forma do programa. Se os nomes existem, se os tipos
  combinam e se o fluxo é válido (um `break` fora de laço, uma função com resultado que
  pode chegar ao fim sem `give`) é a análise semântica que decide; ver "O que o parser
  não verifica".

## Regras

```text
programa    = { função } FIM
função      = "fun" NOME "(" [ parâmetros ] ")" [ "->" tipo ] QUEBRA bloco
parâmetros  = parâmetro { "," parâmetro } [ "," ]
parâmetro   = NOME ":" tipo
tipo        = NOME
bloco       = INDENT instrução { instrução } DEDENT

instrução   = let | atribuição | if | while | loop
            | "break" QUEBRA | "continue" QUEBRA | give | chamada QUEBRA
let         = "let" [ "mut" ] NOME [ ":" tipo ] "=" expressão QUEBRA
atribuição  = NOME "=" expressão QUEBRA
if          = "if" expressão QUEBRA bloco [ "else" ( if | QUEBRA bloco ) ]
while       = "while" expressão QUEBRA bloco
loop        = "loop" QUEBRA bloco
give        = "give" [ expressão ] QUEBRA

expressão   = ou
ou          = e { "or" e }
e           = negação { "and" negação }
negação     = "not" negação | comparação
comparação  = soma [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" ) soma ]
soma        = produto { ( "+" | "-" ) produto }
produto     = unário { ( "*" | "/" | "%" ) unário }
unário      = "-" unário | primária
primária    = INTEIRO | TEXTO | "true" | "false"
            | NOME | chamada | "(" expressão ")"
chamada     = NOME "(" [ argumentos ] ")"
argumentos  = expressão { "," expressão } [ "," ]
```

## Precedência

Da mais fraca para a mais forte:

| Nível | Operadores | Associatividade |
|---|---|---|
| 1 | `or` | esquerda |
| 2 | `and` | esquerda |
| 3 | `not` (prefixo) | — |
| 4 | `==` `!=` `<` `<=` `>` `>=` | não associativa |
| 5 | `+` `-` | esquerda |
| 6 | `*` `/` `%` | esquerda |
| 7 | `-` (prefixo) | — |
| 8 | chamada, `( )`, literais, nomes | — |

```text
not a == b      # not (a == b)
-a * b          # (-a) * b
a or b and c    # a or (b and c)
a < b < c       # erro de sintaxe
a == b != c     # erro de sintaxe
(a < b) < c     # aceito pelo parser; a compatibilidade dos tipos é da análise semântica
```

## Da gramática à árvore

O parser tem uma função por nível de precedência (`or`, `and`, `not`, `comparison`,
`additive`, `multiplicative`, `negation` e `primary`), e cada uma lê os seus operandos
chamando a do nível seguinte, mais forte. A árvore (`src/ast.rs`) guarda a estrutura de
que as etapas seguintes precisam, e não o caminho pelas regras:

- Nem toda regra vira um nó. `expressão = ou` não deixa nada na árvore, e uma `soma` sem
  `+` nem `-` é só o seu `produto`. Um nó `Binária` só aparece onde há um operador.
- Os parênteses mudam a árvore, mas não viram nó: `primary` lê a expressão de dentro e a
  devolve como está. Não existe um nó de agrupamento: o agrupamento fica na própria forma
  da árvore.
- A precedência decide quem é operando de quem. Em `1 + 2 * 3`, `additive` pede o
  operando da direita a `multiplicative`, que consome `2 * 3` inteiro.
- A associatividade decide para que lado a árvore cresce. A repetição `{ ... }` de
  `soma` e de `produto` é um laço que, a cada volta, junta a árvore já montada, à
  esquerda, com o operando seguinte: `10 - 3 - 2` é `(10 - 3) - 2`. A comparação usa
  `[ ... ]` e não repete: uma segunda comparação seguida é erro.

Para ilustrar, a forma das árvores em notação compacta, a mesma que os testes de
`tests/parser.rs` usam para conferi-las. Não é um formato que a CLI aceite ou produza:

| Expressão | Árvore | Resultado |
|---|---|---|
| `1 + 2 * 3` | `(+ 1 (* 2 3))` | 7 |
| `(1 + 2) * 3` | `(* (+ 1 2) 3)` | 9 |
| `10 - 3 - 2` | `(- (- 10 3) 2)` | 5 |
| `10 - (3 - 2)` | `(- 10 (- 3 2))` | 9 |

### Os nós em Rust

Os nós são `enum`s do Rust. Trecho de `src/ast.rs`:

```rust
pub enum ExpressionKind {
    /// Inteiro como foi escrito; a conversão e a faixa ficam para a análise semântica.
    Integer {
        base: Base,
        digits: String,
    },
    Text(String),
    Bool(bool),
    Name(String),
    Call(Call),
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
}
```

- Cada variante tem os seus campos: um `Binary` sempre tem `operator`, `left` e `right`,
  e um `Integer` guarda a base e os dígitos como foram escritos.
- Cada operação sobre a árvore é uma função que usa `match` nas variantes: `ast::render`
  escreve o `.a`, e `semantic::check` produz a árvore verificada.
- Quando um `match` lista as variantes uma a uma, o compilador recusa o código se faltar
  uma; uma variante nova aponta cada um desses lugares. Um braço genérico (`_`) aceita
  qualquer caso, inclusive um que ainda não existe. Em `src/lower.rs`, os operadores
  binários que não são `and`, `or`, `==`, `!=` nem o `+` de textos caem num `_` que emite
  uma instrução de inteiros: um operador novo cairia ali sem aviso do compilador. O
  verificador do `.s` (`src/verifier.rs`) lista os operadores um a um.
- A exaustividade ajuda, mas não cumpre sozinha as obrigações de uma construção nova:
  ela só alcança os `match` explícitos. A leitura e a escrita dos formatos, as regras da
  análise semântica, os testes e a documentação continuam por conta de quem acrescenta.
  E o custo fica de um lado só: uma operação nova é uma função nova, mas uma variante
  nova toca todas as funções que já existem. O "expression problem" continua.

## Decisões

- **`else if`** encadeia condições sem outra palavra-chave e sem outro nível de
  indentação. Não existe `elif`.
- **A atribuição é uma instrução, não uma expressão:** `if x = 1` é erro de sintaxe.
- **Só uma chamada pode ser usada sozinha como instrução.** Uma linha como `a + b`, cujo
  valor seria descartado, é erro de sintaxe.
- **Chamadas só por nome:** `f(x)`. Formas como `factory()(10)` e `object.method()` ficam
  para depois; funções ainda não são valores.
- **Blocos não podem ser vazios** e não há `pass`. `give` serve como corpo mínimo.
- **Uma função sem resultado** termina ao chegar ao fim do corpo; `give` sem valor
  permite encerrá-la antes.
- **Vírgula final** é aceita em parâmetros e argumentos.
- **Só argumentos posicionais.** Argumentos nomeados ficam para quando houver estruturas.
- **Indentação fora de lugar** é erro do parser. O lexer só reconhece que houve
  indentação; o parser decide se ela é permitida ali:

  ```text
  let value = 1
      say(value)    # erro: indentação inesperada
  ```

- **Fora da primeira versão,** para manter o recorte pequeno: `for`/`in`, `fixed`,
  `package`, `bring`, `public`, `shape`, `enum`, `match` e as demais construções do
  vocabulário que não aparecem nas regras acima.

## O que o parser não verifica

O parser verifica só a estrutura. Ficam para a análise semântica, entre outras coisas:

- se os nomes existem e se os tipos são compatíveis;
- `break` e `continue` fora de laço (`fun main()` seguido de `break` é uma AST válida);
- `give` com ou sem valor de acordo com o tipo de retorno da função;
- a faixa dos literais inteiros, incluindo `-9223372036854775808`.

## Operandos ausentes

Quando o parser espera um operando e encontra outra coisa, o diagnóstico diz o que falta.
Ele olha o token encontrado e o operador que veio logo antes:

| Situação | Exemplo | Mensagem |
|---|---|---|
| Operador só binário (`*` `/` `%` `==` `!=` `<` `<=` `>` `>=` `and` `or`) onde começa um operando | `say(* 2)` | `falta o operando à esquerda de '*'` |
| `+` como prefixo | `let x = +1` | `a Pinker não tem '+' unário: '+' precisa de um operando à esquerda` |
| Operador seguido do fim da linha, de `)` ou de `,` | `let x = 1 +` | `falta o operando à direita de '+', encontrei o fim da linha` |
| `not` como operando de um operador de nível mais alto | `1 == not true` | `'not' tem precedência menor que '==' e só pode ser operando dele entre parênteses` |
| Outro token depois de um operador | `1 + if` | `esperava o operando à direita de '+', encontrei 'if'` |

```text
erro.pk:2:16: erro: falta o operando à direita de '+', encontrei o fim da linha
  2 |     let x = 1 +
    |                ^
```

- `-` e `not` são prefixos válidos (`-1`, `--1`, `1 - -2`, `not not true`). O `+` só
  existe entre dois operandos: `+1` é recusado, e não vira `1`.
- Em dois operadores seguidos, como `1 + * 2`, o erro aponta o segundo, que não tem
  operando à esquerda. O parser não tenta adivinhar qual dos dois sobra.
- Os operandos ausentes são erros de forma. `1 + true` tem a forma certa e vira uma
  árvore; é a análise semântica que recusa os tipos.
- Num `.pk`, o lexer fecha cada linha lógica com `QUEBRA`. Dentro de parênteses ou
  colchetes a linha continua, mas um delimitador aberto no fim do arquivo é recusado pelo
  próprio lexer. Por isso, um operador seguido do fim do bloco ou do fim do arquivo só
  chega ao parser por um `.t` editado, e a mensagem diz "o fim do bloco" ou "o fim do
  arquivo".
- O parser para no primeiro erro: não continua a leitura, não entrega uma árvore parcial
  às etapas seguintes, e `--emit=ast` não grava nada. Relatar vários erros de uma vez
  exige uma recuperação que ainda não existe.

## `--emit=ast`

`pinker <arquivo.pk> --emit=ast` grava a árvore em `<arquivo>.a`: uma linha por nó, com a
posição e a descrição, e os filhos indentados sob o pai. A posição de uma expressão
binária é a do operador; os demais nós começam no primeiro token. Os parênteses não
aparecem na árvore: o agrupamento está na própria forma dela. Nomes e tipos levam a
própria posição em `@linha:coluna`.

```text
# pinker ast 1 · fonte: teste.pk
#POSIÇÃO NÓ
1:1      Função main@1:5
2:5        Bloco
2:5          Let mut n@2:13
2:17           Unária -
2:18             Inteiro 1 (base 10)
```

O arquivo `.a` também serve de entrada para a análise semântica (ver
[`arquivos.md`](arquivos.md)).

## Do código à máquina de pilha

[`exemplos/expressoes.pk`](../exemplos/expressoes.pk) mostra as quatro expressões da
tabela acima e o curto-circuito de `and`. Cada etapa do pipeline tem uma fronteira:

- **AST** (`ast::Program`, `.a`): só a estrutura sintática. Os nomes são texto, os
  inteiros guardam base e dígitos, e não há tipos.
- **Árvore verificada** (`checked::CheckedProgram`, `.s`): a mesma forma, com o tipo de
  cada expressão (`: i64`) e os vínculos resolvidos. Cada variável vira um número
  (`nome#n`), cada chamada aponta para a sua função ou para uma predefinida, e os
  inteiros viram valores. Um `-` aplicado a um literal inteiro entra no próprio literal:
  `-(2)` vira `Inteiro -2`.
- **Bytecode** (`bytecode::BytecodeProgram`, `.b`): não há mais árvore. Cada função é
  uma lista de instruções da máquina de pilha, cada uma com a posição de onde veio.

Na raiz do repositório:

```text
cargo run -q -- exemplos/expressoes.pk
cargo run -q -- exemplos/expressoes.pk --emit=ast -o -
cargo run -q -- exemplos/expressoes.pk --emit=semantic -o -
cargo run -q -- exemplos/expressoes.pk --emit=bytecode -o -
```

O primeiro executa o programa e escreve `7`, `9`, `5`, `9`, `false`,
`avaliou o lado direito` e `true`, uma linha cada. Os outros escrevem as etapas na saída
padrão. A seguir, `say(1 + 2 * 3)`, da linha 10, em cada uma.

Trecho do `.a`:

```text
10:5         Chamada say
10:11          Binária +
10:9             Inteiro 1 (base 10)
10:15            Binária *
10:13              Inteiro 2 (base 10)
10:17              Inteiro 3 (base 10)
```

Trecho do `.s`:

```text
10:5         Chamada say (predefinida)
10:11          Binária +: i64
10:9             Inteiro 1: i64
10:15            Binária *: i64
10:13              Inteiro 2: i64
10:17              Inteiro 3: i64
```

Trecho do `.b`, no código da `main`:

```text
0   10:9     const.i64 1
1   10:13    const.i64 2
2   10:17    const.i64 3
3   10:15    mul.i64
4   10:11    add.i64
5   10:5     say.i64
```

A tradução percorre a árvore em pós-ordem: o operando da esquerda, o da direita e depois
o operador. O `*` está mais fundo na árvore, então a sua instrução vem antes da do `+`, e
cada instrução guarda a posição do nó que a gerou. A pilha da chamada, depois de cada
instrução:

| # | Instrução | Pilha (topo à direita) |
|---|---|---|
| 0 | `const.i64 1` | `1` |
| 1 | `const.i64 2` | `1 2` |
| 2 | `const.i64 3` | `1 2 3` |
| 3 | `mul.i64` | `1 6` |
| 4 | `add.i64` | `7` |
| 5 | `say.i64` | vazia; escreve `7` |

Em `(1 + 2) * 3` (instruções 6 a 10), a mesma tradução dá `const.i64 1`, `const.i64 2`,
`add.i64`, `const.i64 3` e `mul.i64`: a soma deixa `3` na pilha antes de o `3` literal
ser empilhado, e `mul.i64` dá 9. A forma
da árvore decide o agrupamento, mas não todos os efeitos da execução: a ordem de
avaliação e o curto-circuito de `and` e `or`, que não seguem só da forma, estão em
[`execucao.md`](execucao.md#ordem-de-avaliação).
