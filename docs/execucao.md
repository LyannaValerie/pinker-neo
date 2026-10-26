# Execução da primeira versão

As decisões de execução do interpretador (`src/interpreter.rs`). Ele percorre a árvore
verificada (`src/checked.rs`) e usa os vínculos e tipos que a análise semântica já
resolveu: não refaz resolução de nomes nem inferência de tipos. As regras de aritmética,
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
  compartilhado por todos os valores que vêm dele. Ele vive enquanto a árvore verificada
  existir, que é a duração da execução. Não é armazenamento `'static` do Rust: é um texto
  com contagem de referências que a árvore mantém.
- Nesta versão, nenhuma operação cria texto novo: todo texto vem de um literal.
- Quando um bloco termina, as variáveis declaradas nele são esvaziadas, por qualquer
  saída: fim normal, `give`, `break`, `continue` ou falha de execução. Quando uma chamada
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
## Limites desta implementação

O interpretador é recursivo: cada chamada, bloco e expressão da Pinker usa pilha do
próprio Rust.

- **Chamadas ativas:** no máximo 10.000 ao mesmo tempo, contando a `main`. O limite
  conta só as chamadas em andamento; as que já terminaram não contam, então um laço pode
  fazer qualquer quantidade de chamadas em sequência. Passar do limite é uma falha de
  execução com diagnóstico e código 3.
- **Pilha:** o pipeline roda numa thread com pilha de 256 MB. Ela foi validada nos
  cenários testados (recursão até o limite, em debug e release), mas não é uma garantia
  geral: uma chamada da Pinker não usa uma quantidade fixa de pilha, e expressões ou
  blocos muito aninhados também consomem pilha. Um estouro da pilha do Rust encerra o
  processo sem diagnóstico da Pinker. Uma pilha explícita, no lugar da recursão, pode ser
  avaliada quando essa limitação importar.
- **Thread:** se a thread não puder ser criada, a CLI informa e termina com código 1.

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

- `pinker <arquivo.pk>` passa por todas as etapas e executa o programa.
- Com `--emit=<etapa>`, o pipeline para na etapa pedida e grava o resultado dela, sem
  executar. Assim, a observação das etapas anteriores continua independente da execução.
