# Arquivos do pipeline

Cada etapa do pipeline grava o seu resultado num arquivo, e a etapa seguinte sabe ler esse
arquivo. Assim, um programa pode atravessar o pipeline inteiro por arquivos, e quem tiver
curiosidade pode inspecionar ou editar o arquivo de qualquer etapa antes de seguir.

```text
pinker ola.pk --emit=tokens     # ola.pk -> ola.t   (lexer)
pinker ola.t  --emit=ast        # ola.t  -> ola.a   (parser)
pinker ola.a  --emit=semantic   # ola.a  -> ola.s   (análise semântica)
pinker ola.s  --emit=bytecode   # ola.s  -> ola.b   (tradução para bytecode)
pinker ola.b                    # executa           (verificador e máquina virtual)
```

`pinker ola.pk` continua rodando tudo de uma vez, na memória. Os dois caminhos dão o mesmo
resultado: cada arquivo gerado a partir do anterior é idêntico ao gerado direto do `.pk`.

| Arquivo | Etapa que grava | Etapa que lê | Conteúdo |
|---|---|---|---|
| `.t` | lexer (`--emit=tokens`) | parser | Os tokens |
| `.a` | parser (`--emit=ast`) | análise semântica | A árvore de sintaxe |
| `.s` | análise semântica (`--emit=semantic`) | verificador do `.s` e tradução para bytecode | A árvore verificada |
| `.b` | tradução para bytecode (`--emit=bytecode`) | verificador do `.b` e máquina virtual | O bytecode |

Sem `--emit`, o programa passa pelas etapas que faltam e executa na máquina virtual,
qualquer que seja a etapa de entrada: `pinker ola.s` também traduz para bytecode e
executa. O interpretador de árvore (`src/interpreter.rs`) não faz parte do caminho da CLI:
fica como referência, e os testes exigem que ele e a máquina virtual concordem (ver
[`execucao.md`](execucao.md)).

As extensões são só convenção. A CLI reconhece o tipo do arquivo pelo cabeçalho.

## Cabeçalho

A primeira linha de cada arquivo identifica o formato, a versão e o código-fonte de
origem:

```text
# pinker tokens 1 · fonte: ola.pk
```

- O formato é `tokens`, `ast`, `semantic` ou `bytecode`.
- A versão é a do formato, e cada formato tem a sua: esta versão do `pinker` lê `tokens`
  1, `ast` 1, `semantic` 2 e `bytecode` 1. Um arquivo de outra versão é recusado com uma mensagem
  clara.
- A fonte é o `.pk` de onde o arquivo veio. Os diagnósticos da linguagem das etapas
  seguintes apontam para ela. Ela é só uma origem declarada: como o resto do arquivo, pode
  ser editada, e nada confere se o arquivo corresponde ao `.pk` atual. Se o `.pk` mudou
  depois que o arquivo foi gerado, as posições dos diagnósticos podem não bater com ele.
- Um arquivo que não começa com `# pinker ` é tratado como código-fonte Pinker. Por isso,
  um `.pk` não deve começar com um comentário `# pinker ...`.

Depois do cabeçalho, linhas que começam com `#` são comentários, e linhas em branco são
ignoradas. Cada tabela começa com um comentário com os nomes das colunas.

## `.t`: tokens

Uma linha por token: a posição (`linha:coluna`), o tipo e a grafia exata no código-fonte.

```text
# pinker tokens 1 · fonte: ola.pk
#POSIÇÃO TIPO           LEXEMA
3:1      PALAVRA        fun
3:5      IDENTIFICADOR  soma
3:9      SÍMBOLO        (
7:25     INTEIRO        0x03  (base 16)
3:32     QUEBRA
4:5      INDENT
```

- Os tipos são `PALAVRA`, `IDENTIFICADOR`, `INTEIRO`, `TEXTO`, `SÍMBOLO`, `QUEBRA`,
  `INDENT`, `DEDENT` e `FIM`. Os quatro últimos não têm grafia.
- As colunas são separadas por espaços; o alinhamento é só para leitura.
- Cada grafia é conferida com as regras do lexer: uma `PALAVRA` precisa ser palavra
  reservada, um `INTEIRO` precisa ter prefixo e dígitos válidos, um `TEXTO` precisa ter
  aspas e escapes válidos.
- A anotação `(base N)` dos inteiros é só informativa; a base vem do prefixo.
- A grafia de um `INTEIRO` mantém o separador de dígitos: `0xFF_FF`.
- O último token precisa ser `FIM`, e nada pode vir depois dele.

O parser confere a estrutura: um `.t` editado com tokens fora de ordem gera os mesmos
erros de sintaxe que o código-fonte equivalente geraria.

## `.a` e `.s`: árvores

Uma linha por nó: a posição, e o nó indentado sob o pai com dois espaços por nível. O nó
começa na coluna seguinte à da posição, que ocupa pelo menos 8 caracteres.

```text
# pinker ast 1 · fonte: ola.pk
#POSIÇÃO NÓ
3:1      Função soma@3:5 -> int@3:29
3:10       Parâmetro a: int@3:13
4:5        Bloco
4:5          Give
4:12           Binária +
4:10             Nome a
4:14             Nome b
```

- Os filhos de cada nó vêm logo abaixo dele, em ordem: num `If`, a condição, o bloco e
  o `Else` opcional; numa `Binária`, o operando da esquerda e depois o da direita; numa
  `Chamada`, os argumentos.
- No `.a`, nomes e tipos levam a própria posição em `@linha:coluna`. A análise semântica
  usa essas posições nos diagnósticos.
- No `.s`, cada expressão termina com o seu tipo (`: i64`) e cada variável aparece como
  `nome#n`. As declarações (`Parâmetro a#0`, `Let n#1`) e as funções (`Função soma`) não
  repetem o tipo, a mutabilidade nem a posição do nome: isso fica na tabela de símbolos
  (abaixo). As chamadas apontam para as funções pelo nome.
- Os textos usam os escapes da Pinker (`\n`, `\t`, `\\`, `\"`).
- Os inteiros do `.a` são escritos com o prefixo da base e sem o separador de dígitos
  (`0xFF_FF` vira `0xFFFF`); os do `.s`, como valor decimal (`-1`).

## `.s`: a tabela de símbolos e a árvore

A análise semântica guarda o que descobre de três maneiras, e o `.s` mostra as três:

- **Atributos nos nós:** o tipo de cada expressão (`: i64`), preenchido depois do
  parsing.
- **Tabela de símbolos:** uma tabela à parte, consultada pelo identificador, que diz a
  que cada nome se refere.
- **Uma estrutura nova:** a árvore verificada, separada da AST, que expressa a semântica
  mais diretamente: `int` já é `i64`, cada uso aponta para a sua declaração.

O arquivo tem três seções, nesta ordem. Cada uma começa com o título entre colchetes:

```text
# pinker semantic 2 · fonte: ola.pk
[funções]
#NOME  DECLARAÇÃO  ASSINATURA
soma   3:5         (i64, i64) -> i64
main   6:5         ()

[variáveis soma]
#N  NOME  TIPO  DECLARAÇÃO  ESPÉCIE
0   a     i64   3:10        parâmetro
1   b     i64   3:18        parâmetro

[variáveis main]
#N  NOME   TIPO  DECLARAÇÃO  ESPÉCIE
0   total  i64   7:9         let

[árvore]
#POSIÇÃO NÓ
3:1      Função soma
3:10       Parâmetro a#0
3:18       Parâmetro b#1
4:5        Bloco
4:5          Give
4:12           Binária +: i64
4:10             Variável a#0: i64
4:14             Variável b#1: i64
6:1      Função main
7:5        Bloco
7:5          Let total#0
...
```

- **`[funções]`:** uma linha por função, na ordem do programa: o nome, a posição do nome
  no código-fonte e a assinatura, `(tipos dos parâmetros) -> resultado`. Sem resultado,
  a assinatura termina no `)`. As chamadas da árvore são resolvidas por esta tabela.
- **`[variáveis <função>]`:** uma seção por função, na mesma ordem da tabela de funções,
  mesmo que a função não tenha variáveis. Uma linha por variável: o número, o nome, o
  tipo, a posição do nome e a espécie (`parâmetro`, `let` ou `let mut`). O número é a
  chave: começa em 0 e segue a ordem das linhas. Dois `let` com o mesmo nome (um
  sombreando o outro) são duas linhas, com números diferentes.
- **`[árvore]`:** as funções na mesma ordem da tabela. Um `nome#n` na árvore se refere à
  linha `n` da tabela da função, e o nome precisa ser o da tabela.
- As colunas são separadas por espaços, e o alinhamento é só para leitura. A última
  coluna de cada linha (assinatura ou espécie) vai até o fim dela.

A tabela é dado, não comentário: o tipo, a mutabilidade e a posição de uma variável vêm
dela. Editar `let mut` para `let` faz o verificador recusar uma atribuição a essa
variável; trocar o tipo faz ele recusar as expressões que não combinam com o tipo novo.

A leitura confere que a árvore concorda com a tabela, e uma discordância é erro de
formato, com a linha do arquivo:

- a tabela de variáveis fora da ordem das funções, ou uma função sem a sua seção;
- uma função da árvore fora da ordem da tabela, ou que não está nela;
- um `nome#n` cujo número não está na tabela, ou cujo nome não é o da tabela;
- uma variável da tabela que a árvore não declara, ou declara mais de uma vez;
- um `Parâmetro` de uma variável que a tabela diz ser `let`, ou o contrário;
- uma assinatura cujos tipos não são os dos parâmetros declarados na árvore.

## `.b`: bytecode

O bytecode é o programa como uma lista de instruções de uma máquina de pilha, por função.
Ele não tem mais a forma do código-fonte: `if`, `while`, `loop`, `break`, `continue` e o
curto-circuito de `and` e `or` viraram saltos. O arquivo tem a mesma tabela de símbolos
do `.s` e, no lugar da árvore, uma seção `[código <função> <posição>]` por função, na
ordem da tabela de funções. A posição do título é a do `fun`.

```text
# pinker bytecode 1 · fonte: ola.pk
[funções]
...
[código soma 3:1]
#N  POSIÇÃO  INSTRUÇÃO
0   4:10     load a#0
1   4:14     load b#1
2   4:12     add.i64
3   4:5      give

[código main 6:1]
#N  POSIÇÃO  INSTRUÇÃO
0   7:22     const.i64 2
1   7:25     const.i64 3
2   7:17     call soma
3   7:5      store total#0
4   8:8      load total#0
5   8:16     const.i64 4
6   8:14     gt.i64
7   8:18     jump_false 13
...
20  7:5      clear total#0
21  6:1      give
```

- Uma linha por instrução: o número (a partir de 0, na ordem das linhas), a posição no
  código-fonte de que ela veio e a instrução. Uma falha de execução aponta essa posição.
- A instrução é um mnemônico, com os tipos depois de pontos, e um operando, se houver.
- Cada chamada tem uma pilha de valores; "empilha" e "desempilha" se referem a ela.
- Os parâmetros são as variáveis da espécie `parâmetro`, na ordem dos números.

| Instrução | Efeito |
|---|---|
| `const.<tipo> <valor>` | Empilha uma constante: `const.i64 -1`, `const.bool true`, `const.text "olá"` |
| `load nome#n` | Empilha o valor da variável |
| `store nome#n` | Desempilha um valor e o guarda na variável |
| `clear nome#n` | Esvazia a variável: o bloco em que ela foi declarada terminou |
| `add` `sub` `mul` `div` `rem` `.<inteiro>` | Desempilha dois inteiros e empilha o resultado, com overflow e divisão por zero conferidos |
| `lt` `le` `gt` `ge` `.<inteiro>` | Desempilha dois inteiros e empilha a comparação |
| `eq` `ne` `.<tipo>` | Desempilha dois valores e empilha a igualdade |
| `neg.<inteiro>` | Troca o sinal de um inteiro com sinal |
| `not` | Nega um `bool` |
| `jump n` | Continua na instrução `n` |
| `jump_false n` | Desempilha um `bool`; se for falso, continua na instrução `n` |
| `call nome` | Desempilha os argumentos e chama a função; o resultado, se houver, fica na pilha |
| `say.<tipo>` | Desempilha um valor e o escreve com uma quebra de linha |
| `morph.<de>.<para>` | Converte um inteiro, se ele couber no tipo de destino |
| `pop` | Descarta o valor do topo: o resultado de uma chamada solta |
| `give` | Encerra a chamada; numa função com resultado, o valor devolvido é o do topo |

A tradução (`src/lower.rs`) segue as mesmas regras do interpretador. Quando um bloco
termina, as variáveis declaradas nele são esvaziadas com `clear`: no fim normal, e antes
do salto de um `break` ou `continue` que sai dele. Uma função sem resultado que chega ao
fim do corpo termina com um `give`. O código que vem depois de um `give`, `break` ou
`continue` no mesmo bloco nunca executa e não é traduzido.

## Diagnósticos

Há dois tipos de erro, e cada um aponta para um lugar diferente:

- **Erro de formato:** uma linha do arquivo não segue o formato. Aponta o próprio arquivo
  e a linha: `ola.a:12: erro de formato: indentação inesperada`. Termina com código 1.
- **Erro da linguagem:** o conteúdo está bem formado, mas o programa tem um erro. Aponta a
  posição no código-fonte de origem, cujo nome vem do cabeçalho:
  `ola.pk:3:5: erro: ...`. É o mesmo diagnóstico do pipeline direto.

`--emit` precisa pedir uma etapa posterior à da entrada: `pinker ola.a --emit=tokens` é
erro de uso, com código 2.

## O verificador do `.s`

Depois do `.s`, nada mais confere o programa, e o interpretador parte do princípio de que
os tipos estão certos. Por isso, ao ler um `.s`, o verificador (`src/verifier.rs`) confere
a árvore antes da execução:

- se existe a `main`, sem parâmetros e sem resultado;
- se o tipo anotado em cada expressão segue a tabela de operadores, a faixa dos literais
  e as assinaturas das funções chamadas;
- se cada variável é usada dentro do seu escopo, depois da declaração, e só recebe
  atribuição se for mutável;
- se `break` e `continue` estão dentro de laços, se cada `give` combina com o resultado
  da função e se uma função com resultado nunca chega ao fim do corpo.

O verificador não infere tipos nem resolve nomes: só confere o que já está anotado. Um
`.s` incoerente é recusado com `fonte:linha:coluna: erro: árvore inconsistente: ...` e
código 1, sem executar nada.

Os arquivos `.t` e `.a` não precisam de verificador: o parser e a análise semântica já
conferem tudo que vem deles.

## O verificador do `.b`

A máquina virtual parte do princípio de que o bytecode está certo: não confere tipos nem
se uma variável tem valor. Ao ler um `.b`, o verificador (`src/bytecode.rs`) acompanha
cada função por todos os caminhos possíveis e calcula, para cada instrução alcançável, os
tipos na pilha e se cada variável está cheia, vazia ou pode estar das duas formas. Ele
recusa:

- uma instrução que desempilha um tipo diferente do que ela pede, ou de uma pilha vazia;
- uma constante fora da faixa do tipo, ou `neg` de um inteiro sem sinal;
- `load` de uma variável que pode estar vazia;
- `store` numa variável imutável que pode já ter um valor. Um `let mut` pode guardar de
  novo; um `let` só guarda quando está vazio. O bytecode não guarda os blocos: um
  `clear` encerra a vida da variável, e um `store` depois dele começa uma vida nova, como
  o `let` de um laço a cada volta. Por isso, um `clear` acrescentado à mão antes de um
  `store` é aceito: o programa continua coerente, como se a variável tivesse sido
  declarada de novo;
- um salto para fora do código, ou uma instrução que pode deixar a execução passar da
  última;
- dois caminhos que chegam à mesma instrução com tipos diferentes na pilha;
- um `give` cuja pilha não seja exatamente o resultado da função (ou vazia, sem
  resultado);
- uma `main` com parâmetros ou resultado.

Um `.b` incoerente é recusado com `fonte:linha:coluna: erro: bytecode inconsistente:
instrução <n>: ...` e código 1, sem executar nada. O bytecode traduzido de um `.s` não
passa por esse verificador na CLI, porque vem de uma árvore já conferida; os testes
conferem que todo bytecode traduzido passa nele.

## `.gitignore`

Os arquivos `*.t`, `*.a`, `*.s` e `*.b` gerados pelo pipeline são ignorados pelo Git. Atenção:
`.s` é também a extensão usual de assembly, e `.a`, a de bibliotecas estáticas do C. Se o
projeto passar a ter arquivos desses tipos, será preciso revisar o `.gitignore` ou as
extensões do pipeline.
