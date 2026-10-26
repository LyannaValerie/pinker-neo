# Decisões da primeira versão

Contrato inicial da primeira versão da Pinker. O princípio é implementar pouco de cada
vez, mas decidir cedo aquilo que condiciona a arquitetura: tipos, memória e significado
das operações.

## Escopo

| Momento | Decisões e entregas |
|---|---|
| **Agora** | Indentação, comentários, literais, tipos, mutabilidade, assinaturas, retorno, overflow e política inicial de memória |
| **Primeira versão** | Funções, variáveis, condições, laços, inteiros, `bool`, texto e saída, com diagnósticos e execução verificável |
| **Depois, conforme a necessidade** | Estruturas, variantes, padrões, resultados e genéricos |
| **Com contrato próprio** | Empréstimos avançados, ponteiros crus, assembly, layout externo e uniões cruas |

## Indentação

- Os blocos são definidos por indentação, sem dois-pontos.
- Só espaços. Tabulação na indentação é erro.
- Cada recuo de volta (dedent) precisa voltar a um nível que já existe; caso contrário,
  é erro.
- A gramática não impõe uma largura de indentação. O formatador usa quatro espaços.
- Uma expressão só continua na linha seguinte dentro de um delimitador aberto que a
  gramática reconheça, como parênteses ou colchetes. Uma linha que termina em operador
  não continua na seguinte.

```text
let total = (
    first
    + second
)
```

## Comentários

`#` inicia um comentário que vai até o fim da linha, exceto dentro de um texto. Nos
arquivos `.pk`, as âncoras de navegação usam essa sintaxe: `# pink-nav: start`.

## Literais

- **Inteiros:** decimal, hexadecimal (`0xFF`) e binário (`0b1010`). Um prefixo sem
  dígitos, como `0x`, é erro.
- **Texto:** entre aspas duplas, com os escapes `\n`, `\t`, `\\` e `\"`. Um escape
  desconhecido é erro.
- **Lógicos:** `true` e `false`.
- O sinal não faz parte do literal: `-1` é a negação aplicada a `1`. A verificação de
  faixa considera a negação, de modo que o mínimo de `i64`, `-9223372036854775808`, é
  aceito.

## Tipos

- **Inteiros de largura explícita:** `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`,
  `u64`.
- **Inteiro padrão:** sem contexto, um literal inteiro tem o tipo `int`. Com contexto,
  como uma anotação, ele assume o tipo pedido, se o valor couber nele; fora da faixa, é
  erro.
- **`int`:** apelido de `i64`, com o mesmo tipo, a mesma faixa e as mesmas operações;
  não é um tipo nominal separado. É uma mudança deliberada em relação à v0, em que o
  `bombom` era um inteiro de 64 bits sem sinal, compatível com `u64`. Um inteiro padrão
  com sinal permite escrever valores negativos, diferenças e contadores decrescentes.
- **`bool`** e **`text`**.

```text
let count = 0          # int
let mask: u64 = 0xFF   # u64, pelo contexto
let delta = -1         # int
let invalid: u8 = 256  # erro: fora da faixa
```

## Variáveis e mutabilidade

- `let` declara uma variável imutável; `let mut`, uma variável que pode ser reatribuída.
- `mut` permite substituir o valor da variável. Não permite alterar o conteúdo de um
  texto.

## Funções

- Tipos explícitos em todos os parâmetros e no retorno. A inferência vale só para
  variáveis locais.
- `-> T` indica que a função devolve um `T`. Sem `->`, a função não devolve resultado.
- `give expressão` devolve exatamente o valor da expressão, que precisa ser compatível
  com o tipo de retorno declarado. Não há embrulho automático: um sucesso é
  `give Result.Ok(valor)`.

## Condições

`if` e `while` exigem um `bool`. Números, textos e ponteiros não viram condições
implicitamente.

## Conversões

- `morph` converte um valor segundo as conversões definidas entre tipos. Não reinterpreta
  memória.
- Na primeira versão, o tipo de destino vem do contexto: `let larger: i64 = morph(value)`.
  Sem um destino determinável, é erro.
- Uma conversão que reduz a faixa nunca trunca o valor em silêncio.
- A sintaxe de genéricos em expressões fica para quando eles forem necessários.

## Overflow

- Quando detectado em avaliação constante, é erro de compilação.
- Quando depende de valores em execução, é uma falha definida em tempo de execução.
- Um valor nunca dá a volta em silêncio.
- O comportamento é o mesmo em debug e em release.

## Memória

- Inteiros, `bool` e `text` se comportam como valores. O texto é imutável.
- Não há referências nem ponteiros públicos na primeira versão.
- **Implementação:** literais de texto ficam em armazenamento estático; textos dinâmicos
  usam contagem de referências. Como um texto não referencia outros objetos, não há
  ciclos, e não é preciso um coletor de lixo geral nem um verificador de empréstimos.
- **Obrigações da implementação:**
  - Retornar ou copiar um texto mantém o armazenamento vivo enquanto houver uso.
  - Perder a última referência libera o armazenamento dinâmico.
  - Uma falha de alocação tem comportamento definido; não vira corrupção nem sucesso
    falso.
- **Limite desta política:** a ausência de ponteiros reduz o que o programa pode fazer
  de errado, mas não prova que o runtime esteja livre de erros de memória.

```text
let first = "Pinker"
let mut second = first

second = "Outra"

say(first)   # Pinker
say(second)  # Outra
```

## Lexer

### Fronteiras

| Responsabilidade | Camada |
|---|---|
| Reconhecer palavras, identificadores, números, textos e comentários | Lexer |
| Produzir quebras lógicas, `INDENT` e `DEDENT` | Lexer |
| Verificar se uma construção permite abrir bloco | Parser |
| Interpretar `-` como negação | Parser |
| Resolver o tipo e verificar a faixa do inteiro | Análise semântica |
| Gerenciar o armazenamento e a contagem de referências dos textos | Runtime |

- O lexer não converte números: guarda a base e os dígitos, para que a análise
  semântica aceite o mínimo de `i64` e toda a faixa de `u64`.
- Tipos (`int`, `i32`, `text`) e intrínsecas (`say`, `morph`) são identificadores.
  O significado deles é resolvido nas etapas seguintes.

### Critério de conclusão

O primeiro lexer está concluído quando produz tokens com localização correta e
diagnósticos verificáveis para entradas válidas e inválidas, incluindo indentação,
continuação entre delimitadores, comentários, escapes e fim de arquivo. Ele não
antecipa o parser, os empréstimos nem as funcionalidades adiadas.

### Escolhas de implementação

Pontos que o contrato não fixava e que o primeiro lexer decidiu. Podem ser revistos.

- O lexer para no primeiro erro.
- A coluna conta caracteres, não bytes.
- Identificadores seguem `[A-Za-z_][A-Za-z0-9_]*` e diferenciam maiúsculas de
  minúsculas. Textos e comentários aceitam Unicode.
- Tabulação fora da indentação conta como espaço. Linhas vazias ou só com comentário
  são ignoradas na indentação, mesmo que tenham tabulação.
- Quebras de linha `\n` e `\r\n` são aceitas; `\r` isolado é erro.
- Um texto não pode atravessar linhas.
- Os prefixos aceitam as duas caixas (`0x`/`0X`, `0b`/`0B`), e os dígitos hexadecimais
  também (`a–f`, `A–F`). Outro prefixo, como `0o`, é erro.
- Zeros à esquerda são aceitos e sempre decimais: `010` é dez, nunca octal.
- Um literal malformado (`0x`, `0b102`, `123abc`) é erro; nunca é dividido em tokens
  que escondam o problema.
- `1.5` é erro: números com parte fracionária ainda não existem.
- `!` sozinho é erro, com a sugestão de usar `not`.
- O próprio lexer aponta `(` ou `[` sem par e fechamentos trocados, como `([)]`, porque
  já mantém uma pilha dos delimitadores abertos para saber quando a linha continua.
- Com erro, a CLI termina com código diferente de zero e não grava o arquivo da etapa.

### `--emit=tokens`

`pinker <arquivo.pk> --emit=tokens` grava os tokens em `<arquivo>.tokens`, ao lado da
entrada. `-o <caminho>` escolhe outro destino, e `-o -` escreve na saída padrão. Sem
`--emit`, o arquivo passa pelo lexer e só os erros são mostrados.

## Adiado

- **`union`:** até haver uma necessidade que `enum` não atenda. Uniões cruas, para
  interfaces C, exigirão contrato próprio de layout, alinhamento e validade. Não é
  palavra reservada, e a grafia não está garantida.
- **`propagate`:** por enquanto, `give Result.Err(...)` já expressa o mesmo. Não é
  palavra reservada, e a grafia não está garantida.
- **Genéricos em expressões.**
- **`unsafe`** (proposto): antes de ponteiros crus e assembly, será preciso definir
  quais operações o exigem, quais verificações continuam valendo dentro dele, como uma
  abstração impede o uso incorreto dos seus detalhes inseguros e quais garantias o
  código seguro recebe. Alocar um objeto gerenciado não precisa de `unsafe`.

## Organização do código

Um único pacote Cargo, com o `Cargo.toml` na raiz e o Rust em `src/`. Workspaces ou
crates adicionais só quando surgir uma necessidade concreta.
