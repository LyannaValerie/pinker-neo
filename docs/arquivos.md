# Arquivos do pipeline

Cada etapa do pipeline grava o seu resultado num arquivo, e a etapa seguinte sabe ler esse
arquivo. Assim, um programa pode atravessar o pipeline inteiro por arquivos, e quem tiver
curiosidade pode inspecionar ou editar o arquivo de qualquer etapa antes de seguir.

```text
pinker ola.pk --emit=tokens     # ola.pk -> ola.t   (lexer)
pinker ola.t  --emit=ast        # ola.t  -> ola.a   (parser)
pinker ola.a  --emit=semantic   # ola.a  -> ola.s   (análise semântica)
pinker ola.s                    # executa           (verificador e interpretador)
```

`pinker ola.pk` continua rodando tudo de uma vez, na memória. Os dois caminhos dão o mesmo
resultado: cada arquivo gerado a partir do anterior é idêntico ao gerado direto do `.pk`.

| Arquivo | Etapa que grava | Etapa que lê | Conteúdo |
|---|---|---|---|
| `.t` | lexer (`--emit=tokens`) | parser | Os tokens |
| `.a` | parser (`--emit=ast`) | análise semântica | A árvore de sintaxe |
| `.s` | análise semântica (`--emit=semantic`) | verificador e interpretador | A árvore verificada |

As extensões são só convenção. A CLI reconhece o tipo do arquivo pelo cabeçalho.

## Cabeçalho

A primeira linha de cada arquivo identifica o formato, a versão e o código-fonte de
origem:

```text
# pinker tokens 1 · fonte: ola.pk
```

- O formato é `tokens`, `ast` ou `semantic`.
- A versão é a do formato; esta versão do `pinker` lê a versão 1. Um arquivo de outra
  versão é recusado com uma mensagem clara.
- A fonte é o `.pk` de onde o arquivo veio. Os diagnósticos da linguagem das etapas
  seguintes apontam para ela.
- Um arquivo que não começa com `# pinker ` é tratado como código-fonte Pinker. Por isso,
  um `.pk` não deve começar com um comentário `# pinker ...`.

Depois do cabeçalho, linhas que começam com `#` são comentários, e linhas em branco são
ignoradas. A segunda linha de cada arquivo é um comentário com os nomes das colunas.

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
- No `.s`, cada expressão termina com o seu tipo (`: i64`), cada variável aparece como
  `nome#n`, e as declarações trazem o tipo: `Parâmetro a#0: i64`, `Let mut n#1@5:9: u8`.
  As chamadas apontam para as funções pelo nome.
- Os textos usam os escapes da Pinker (`\n`, `\t`, `\\`, `\"`).
- Os inteiros do `.a` são escritos com o prefixo da base (`0x03`); os do `.s`, como valor
  decimal (`-1`).

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

## `.gitignore`

Os arquivos `*.t`, `*.a` e `*.s` gerados pelo pipeline são ignorados pelo Git. Atenção:
`.s` é também a extensão usual de assembly, e `.a`, a de bibliotecas estáticas do C. Se o
projeto passar a ter arquivos desses tipos, será preciso revisar o `.gitignore` ou as
extensões do pipeline.
