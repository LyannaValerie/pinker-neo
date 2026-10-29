# Execução da primeira versão

As decisões de execução da Pinker. Há dois motores, e as regras valem para os dois:

- **A máquina virtual** (`src/vm.rs`) executa o bytecode traduzido da árvore verificada
  (`src/lower.rs`). É o caminho padrão da CLI.
- **O interpretador de árvore** (`src/interpreter.rs`) percorre a árvore verificada
  (`src/checked.rs`) diretamente. Fica como referência: os testes exigem que os dois
  deem a mesma saída, o mesmo código de término e o mesmo diagnóstico, com a mesma
  mensagem e a mesma posição.

Os dois usam o mesmo **runtime** (`src/runtime.rs`): os valores em execução, as
operações sobre eles (aritmética conferida, comparações, negação, `morph`, `say`), as
falhas definidas com as suas mensagens e o limite de chamadas. Uma regra de execução é
escrita uma vez só, e por isso não pode divergir entre os motores.

Nenhum dos dois refaz resolução de nomes nem inferência de tipos: usam os vínculos e
tipos que a análise semântica já resolveu. As regras de aritmética,
conversão, `say` e `main` já estão em [`decisoes.md`](decisoes.md) e
[`semantica.md`](semantica.md); aqui ficam só as que faltavam.

## Ordem de avaliação

- Da esquerda para a direita, sempre.
- Numa expressão binária, o operando da esquerda é avaliado por inteiro antes do da
  direita.
- Os argumentos de uma chamada são avaliados da esquerda para a direita, antes da
  chamada.
- `and` e `or` fazem curto-circuito: o operando da direita só é avaliado se o da esquerda
  não decidir o resultado.
- Em `let` e na atribuição, o valor é avaliado antes de ser guardado na variável.
- Numa divisão, os dois operandos são avaliados antes de se conferir se o divisor é zero.

```text
soma(a(), b())   # a() executa antes de b()
a() + b()        # a() executa antes de b()
```

## Representação dos valores

| Tipo | Representação |
|---|---|
| Inteiros (`i8` a `u64`) | Um inteiro de 128 bits, sempre dentro da faixa do tipo estático |
| `bool` | Verdadeiro ou falso |
| `text` | Uma referência compartilhada, com contagem, a um texto imutável |

- O tipo de cada valor é o tipo estático da expressão; o valor não carrega o tipo.
- Cada operação inteira é feita com operações verificadas de 128 bits, e o resultado é
  conferido contra a faixa do tipo da expressão. Fora da faixa, é falha de execução. Se
  nem o resultado intermediário couber em 128 bits (o produto de dois `u64` máximos, por
  exemplo), também é overflow, com diagnóstico, nunca pânico nem resultado diferente
  entre debug e release.
- Cada chamada tem um quadro com uma posição para cada variável da função, indexada pelo
  número que a análise semântica deu à declaração. Uma variável sombreada e a que a
  sombreia ocupam posições diferentes.
- Argumentos e atribuições passam valores. Um texto passado ou copiado compartilha o
  mesmo armazenamento, o que não se percebe porque o texto é imutável.

## Gestão dos textos

- Cada literal de texto tem um único armazenamento, criado na análise semântica e
  compartilhado por todos os valores que vêm dele, inclusive pela instrução `const.text`
  do bytecode traduzido. Ele vive enquanto a árvore verificada ou o bytecode existir, que
  é a duração da execução. Um `.b` lido de um arquivo cria o armazenamento de cada
  constante de texto ao ser lido. Não é armazenamento `'static` do Rust: é um texto
  com contagem de referências que a árvore mantém.
- Nesta versão, nenhuma operação cria texto novo: todo texto vem de um literal.
- Quando um bloco termina, as variáveis declaradas nele são esvaziadas, por qualquer
  saída: fim normal, `give`, `break`, `continue` ou falha de execução. No bytecode, o fim
  normal e as saídas por `break` e `continue` esvaziam com `clear`; `give` e a falha
  descartam o quadro inteiro. Quando uma chamada
  termina, o quadro inteiro é descartado. Cada uma dessas perdas solta uma referência.
- O valor de um `give` já tem a sua própria referência antes da limpeza: um texto
  devolvido chega intacto a quem chamou.
- O armazenamento de um texto criado durante a execução seria liberado ao perder a
  última referência; os literais continuam vivos com a árvore.

## Falhas de execução

São falhas definidas de execução:

| Falha | Mensagem |
|---|---|
| Overflow aritmético ou de negação | `overflow: 255 + 1 = 256 não cabe em u8` |
| Divisão ou resto por zero | `divisão por zero` |
| Conversão fora da faixa | `conversão fora da faixa: 300 não cabe em u8` |
| Chamadas ativas acima do limite | `limite de 10000 chamadas aninhadas excedido` |
| Erro ao escrever a saída de `say` | `não foi possível escrever a saída: ...` |

- O diagnóstico segue o formato dos outros: `arquivo:linha:coluna: erro de execução:
  mensagem`. A posição é a do operador, da conversão ou da chamada que falhou.
- A falha encerra o programa na hora, sem executar mais nada.
- A saída que o programa já escreveu antes da falha é preservada e aparece antes do
  diagnóstico.
- A saída passa por um buffer: um `say` pode ser aceito e a escrita só falhar quando o
  buffer é descarregado. Por isso, ao fim de uma execução normal, a saída é descarregada,
  e uma falha nesse momento é uma falha de execução, apontada na posição da `main`. O
  programa nunca termina com sucesso sem ter entregado a saída.
- Se o programa falhou e a saída escrita antes da falha também não pôde ser entregue, o
  diagnóstico da falha é mantido, seguido de uma linha que informa a perda da saída.
  Quando a falha é a da própria saída, ela aparece uma vez só.
## Limites desta implementação

A máquina virtual é iterativa: as chamadas da Pinker ficam numa lista de quadros, não na
pilha do Rust. O interpretador de árvore é recursivo: cada chamada, bloco e expressão da
Pinker usa pilha do próprio Rust. As etapas anteriores à execução (parser, análise
semântica, verificadores e tradução para bytecode) também são recursivas, na proporção do
aninhamento do programa.

- **Chamadas ativas:** no máximo 10.000 ao mesmo tempo, contando a `main`, nos dois
  motores. A máquina virtual não depende da pilha do Rust para isso, mas mantém o limite
  para dar o mesmo resultado que o interpretador. O limite
  conta só as chamadas em andamento; as que já terminaram não contam, então um laço pode
  fazer qualquer quantidade de chamadas em sequência. Passar do limite é uma falha de
  execução com diagnóstico e código 3.
- **Pilha:** o pipeline roda numa thread com pilha de 256 MB. Para o interpretador, ela
  foi validada nos cenários testados (recursão até o limite, em debug e release), mas não
  é uma garantia geral: uma chamada da Pinker não usa uma quantidade fixa de pilha, e expressões ou
  blocos muito aninhados também consomem pilha. Um estouro da pilha do Rust encerra o
  processo sem diagnóstico da Pinker. Uma pilha explícita, no lugar da recursão, pode ser
  avaliada quando essa limitação importar.
- **Thread:** se a thread não puder ser criada, a CLI informa e termina com código 1.

## Desempenho

Medição de 2026-10-28, com `cargo test --release --test desempenho -- --ignored
--nocapture` (`tests/desempenho.rs`): cinco programas, cada um executado uma vez para
aquecer e cinco vezes medidas em cada motor. O tempo é a mediana, só da execução (a
tradução para bytecode fica fora), e os dois motores precisam dar a mesma saída. Máquina:
AMD Ryzen 7 5700X, Linux no WSL2, `rustc` 1.99.0.

| Programa | Interpretador (ms) | VM (ms) | VM / interpretador |
|---|---:|---:|---:|
| Recursão (Fibonacci 27) | 87 | 67 | 0,76 |
| Laço aritmético (3 milhões de voltas) | 411 | 430 | 1,05 |
| Laços aninhados (primos abaixo de 30 mil) | 77 | 79 | 1,03 |
| Laços longos com `u64` (Collatz até 30 mil) | 452 | 460 | 1,02 |
| Cópia de textos (1 milhão de voltas) | 186 | 195 | 1,05 |

A tabela é uma de três execuções completas. Nas três, a razão VM / interpretador ficou
entre 0,76 e 0,77 na recursão e entre 0,99 e 1,07 nos outros quatro programas.

- Nestes programas, a VM não é mais rápida que o interpretador de árvore: nos laços, leva
  o mesmo tempo ou até 7% mais, e só nas chamadas recursivas leva menos, cerca de 24%
  menos tempo.
- Os números valem para esta máquina e estes programas; não são uma garantia geral.
- Não houve perfil de execução. Uma hipótese a conferir antes de otimizar: o custo está
  no que os dois motores compartilham, como a aritmética em 128 bits com faixa conferida
  e os valores num `enum` com textos contados, e não no despacho das instruções.

## Falhas fatais do ambiente

| Situação | Garantia |
|---|---|
| Falha de execução tratada pela Pinker | Diagnóstico com posição e código 3 |
| Esgotamento de memória, tratado pelo Rust | Término fatal; sem garantia de diagnóstico da Pinker nem do código 3 |
| Estouro da pilha do Rust | Término fatal; sem garantia de diagnóstico da Pinker nem do código 3 |

Nos três casos, a falha nunca vira corrupção silenciosa nem sucesso falso.

## Códigos de saída

| Código | Significado |
|---|---|
| 0 | `main` terminou normalmente |
| 1 | Erro de compilação (lexer, parser ou análise semântica), de leitura e gravação de arquivos, ou falha ao criar a thread de execução |
| 2 | Uso incorreto da linha de comando |
| 3 | Falha definida de execução |
| 101 | Defeito interno do `pinker` (um pânico do Rust); a mensagem do pânico é escrita antes |

## Linha de comando

- `pinker <arquivo.pk>` passa por todas as etapas e executa o programa na máquina
  virtual.
- `pinker <arquivo.b>` executa um bytecode gravado antes, depois de conferi-lo com o
  verificador do `.b`. `pinker <arquivo.s>` confere a árvore verificada com o verificador
  do `.s`, traduz para bytecode e executa (ver [`arquivos.md`](arquivos.md)). Também é
  possível partir de um `.t` ou de um `.a`: o pipeline começa na etapa seguinte à do
  arquivo.
- Com `--emit=<etapa>`, o pipeline para na etapa pedida e grava o resultado dela, sem
  executar. Assim, a observação das etapas anteriores continua independente da execução.
