# Gramática da primeira versão

A gramática do subconjunto inicial da Pinker, que o parser (`src/parser.rs`) implementa.
Cada regra corresponde a uma função do parser.

Os nomes em maiúsculas são tokens do lexer: `NOME` (identificador), `INTEIRO`, `TEXTO`,
`QUEBRA` (fim de uma linha lógica), `INDENT`, `DEDENT` e `FIM`. Os demais aparecem entre
aspas, como no código.

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

## `--emit=ast`

`pinker <arquivo.pk> --emit=ast` grava a árvore em `<arquivo>.ast`: uma linha por nó, com
a posição e a descrição, e os filhos indentados sob o pai. A posição de uma expressão
binária é a do operador; os demais nós começam no primeiro token. Os parênteses não
aparecem na árvore: o agrupamento está na própria forma dela.

```text
POSIÇÃO  NÓ
1:1      Função main
2:5        Bloco
2:5          Let mut n
2:17           Unária -
2:18             Inteiro 1 (base 10)
```
