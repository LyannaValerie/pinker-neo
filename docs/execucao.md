# Execução da primeira versão

As decisões de execução da Pinker. Há dois motores, e as regras valem para os dois:

- **A máquina virtual** (`src/vm.rs`) executa o bytecode traduzido da árvore verificada
  (`src/lower.rs`). É o caminho padrão da CLI.
- **O interpretador de árvore** (`src/interpreter.rs`) percorre a árvore verificada
  (`src/checked.rs`) diretamente. Fica como referência: os testes exigem que os dois
  deem a mesma saída, o mesmo código de término e o mesmo diagnóstico, com a mesma
  mensagem e a mesma posição.

Os dois usam o mesmo **runtime** (`src/runtime.rs`): os valores em execução, as
operações sobre eles (aritmética conferida, comparações, negação, concatenação de
textos, `morph`, `say`), as falhas definidas com as suas mensagens e o limite de
chamadas. Cada operação fica escrita num lugar só. Isso não basta para os motores
concordarem: a ordem de avaliação, o fluxo, a gestão dos quadros e a tradução para
bytecode são de cada um. Por isso os testes executam os dois motores e os comparam
entre si e com resultados esperados escritos à parte, já que dois motores que
compartilham um erro do runtime concordariam no erro.

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

### Estrutura e ordem

A árvore decide o agrupamento: quem é operando de quem (ver "Da gramática à árvore" em
[`gramatica.md`](gramatica.md)). Ela não decide sozinha o que acontece na execução. A
ordem dos efeitos vem das regras acima, e o tipo, da análise semântica: a mesma forma
`a + b` é uma soma de inteiros ou uma concatenação de textos.

Na aritmética, a tradução para bytecode (`src/lower.rs`) é pós-fixada: o operando da
esquerda, o da direita e o operador. Como a máquina executa as instruções na ordem, isso
já dá a avaliação da esquerda para a direita. Em `and` e `or`, a mesma tradução estaria
errada, porque avaliaria sempre o operando da direita. A tradução usa saltos. Em
[`exemplos/expressoes.pk`](../exemplos/expressoes.pk), as linhas 16 e 17 são
`say(false and avisa())` e `say(true and avisa())`. Com

```text
cargo run -q -- exemplos/expressoes.pk --emit=bytecode -o -
```

este é o trecho delas no código da `main`:

```text
24  16:9     const.bool false
25  16:15    jump_false 28
26  16:19    call avisa
27  16:15    jump 29
28  16:15    const.bool false
29  16:5     say.bool
30  17:9     const.bool true
31  17:14    jump_false 34
32  17:18    call avisa
33  17:14    jump 35
34  17:14    const.bool false
35  17:5     say.bool
```

- Na linha 16, `jump_false 28` desempilha o `false` e salta para a instrução 28, que
  empilha o resultado, `false`. A instrução 26, `call avisa`, não executa.
- Na linha 17, o `true` não salta: `call avisa` executa, e o resultado dela é o do
  `and`; `jump 35` pula o `const.bool false`.
- `or` faz o contrário: se o operando da esquerda for verdadeiro, o resultado é `true`
  sem avaliar o da direita.

Por isso a linha `avaliou o lado direito`, que `avisa` escreve, aparece uma vez só na
saída do exemplo: a da linha 17. Os testes
conferem esse efeito: `exemplos/expressoes.pk` está em `tests/exemplos.rs`, e
`curto_circuito_nao_avalia_o_lado_direito`, em `tests/interpreter.rs`, faz o mesmo com
`and` e `or` nos dois motores. Os dois testes comparam a saída com um resultado esperado
escrito à parte.

## Representação dos valores

| Tipo | Representação |
|---|---|
| Inteiros (`i8` a `u64`) | Um inteiro de 128 bits, sempre dentro da faixa do tipo estático |
| `bool` | Verdadeiro ou falso |
| `text` | Uma referência compartilhada, com contagem, a um texto imutável |

- Um literal é o que está escrito no código (`0xFF`, `"olá"`). A análise semântica o
  converte num valor do tipo estático da expressão. Na execução, um valor é um `Value`
  (`src/runtime.rs`), com as variantes `Int`, `Bool` e `Text`. A variante separa
  inteiros, lógicos e textos, mas um `Int` não carrega a largura nem o sinal do seu
  tipo: `255` num `u8` e `255` num `i64` são o mesmo `Value::Int(255)`. O tipo inteiro
  vem da expressão, resolvido pela análise semântica. Cada operação o recebe (no
  bytecode, no próprio mnemônico, como `add.u8`) e confere a faixa por ele.
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
- A concatenação (`+` entre dois textos) é a única operação que cria um texto durante a
  execução. Ela avalia o operando da esquerda e depois o da direita, cada um uma vez, e
  cria um armazenamento novo com o conteúdo dos dois, nessa ordem, byte a byte: Unicode
  e escapes são preservados, sem normalização. O resultado é imutável, como qualquer
  texto, e os operandos não mudam. Mesmo com um operando vazio, o resultado é um
  armazenamento novo.
- O texto criado é **dinâmico**: o armazenamento tem contagem de referências e não
  pertence à árvore nem ao bytecode. Copiá-lo para outra variável, passá-lo como
  argumento ou devolvê-lo com `give` acrescenta uma referência; ele continua válido
  enquanto houver uma.
- Quando um bloco termina, as variáveis declaradas nele são esvaziadas, por qualquer
  saída: fim normal, `give`, `break`, `continue` ou falha de execução. No bytecode, o fim
  normal e as saídas por `break` e `continue` esvaziam com `clear`; `give` e a falha
  descartam o quadro inteiro. Quando uma chamada
  termina, o quadro inteiro é descartado. Cada uma dessas perdas solta uma referência.
- O valor de um `give` já tem a sua própria referência antes da limpeza: um texto
  devolvido chega intacto a quem chamou.
- O armazenamento de um texto dinâmico é liberado no momento em que ele perde a última
  referência: quando o bloco da variável que o guardava termina, quando a variável
  recebe outro valor, quando o quadro de uma chamada é descartado ou quando um valor
  intermediário é consumido. Os literais continuam vivos com a árvore ou com o
  bytecode.
- Criar um texto pode faltar memória. Isso não é uma falha definida de execução: é o
  esgotamento de memória tratado pelo Rust, um término fatal (ver "Falhas fatais do
  ambiente"). Nunca vira corrupção nem sucesso falso, mas não há diagnóstico da Pinker
  nem código 3.

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
  mensagem`. A posição é a do operador, da conversão ou da chamada que falhou. Quando a
  entrada é o código-fonte, a linha e um `^` vêm embaixo (ver "O trecho do
  código-fonte" em [`arquivos.md`](arquivos.md)).
- A falha encerra o programa na hora, sem executar mais nada. Isso vale também dentro
  de uma expressão: se a avaliação de um operando ou de um argumento falha, nada depois
  dela é avaliado. O outro operando, os argumentos seguintes e a operação ou a chamada
  que esperava o valor não executam. A regra de avaliar os dois operandos de uma divisão
  antes de conferir o divisor vale quando os dois terminam. A posição é a da operação que
  falhou, mesmo quando ela está em outra função.

  ```text
  fun marca(t: text, n: int) -> int
      say(t)
      give n

  fun main()
      say("antes")
      say(marca("a", 1) + marca("b", 6) / marca("c", 0))
      say("depois")
  ```

  A saída padrão recebe `antes`, `a`, `b` e `c`, uma por linha. A soma e o
  `say("depois")` não executam. O stderr recebe o diagnóstico, e o código é 3:

  ```text
  falha.pk:7:39: erro de execução: divisão por zero
    7 |     say(marca("a", 1) + marca("b", 6) / marca("c", 0))
      |                                       ^
  ```
- Os tipos estáticos não eliminam essas falhas. Eles garantem que `/` recebe dois
  inteiros do mesmo tipo, mas não que o divisor seja diferente de zero, que o resultado
  caiba no tipo nem que a saída aceite a escrita. Quando a análise semântica calcula o
  divisor como zero (`x / 0`, `5 % (2 - 2)`), o erro é de compilação, antes da execução.
  Com valores que só existem na execução, é uma falha de execução.
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
