# Visão da Pinker

> "O poder do C, a segurança do Rust e a simplicidade do Python."

## O que é a Pinker

A Pinker é uma linguagem de programação **estática** com sintaxe inspirada em Python.
Ela oferece recursos comuns às linguagens de alto nível, mas suas principais
capacidades estão no **baixo nível**.

## Objetivo

Uma linguagem focada em **segurança**, **velocidade** e **criação**, tanto no baixo
quanto no alto nível.

O vocabulário da linguagem (palavras reservadas, tipos e intrínsecas) está em
[`palavras.md`](palavras.md). As decisões da primeira versão estão em
[`decisoes.md`](decisoes.md), a gramática, em [`gramatica.md`](gramatica.md), as regras da análise semântica, em
[`semantica.md`](semantica.md), as de execução, em [`execucao.md`](execucao.md), e os arquivos de cada etapa, em
[`arquivos.md`](arquivos.md).

## Para que ela existe

A Pinker é a base de um projeto maior, construído em camadas:

1. **O computador.** Antes do sistema operacional, construiremos a própria máquina:
   memória, armazenamento, tudo.
2. **Pinker Next.** Uma segunda linguagem, **dinâmica**, escrita em Pinker e mais
   próxima de Python ou Ruby do que de C. Ela herda a filosofia da Pinker.
3. **O sistema operacional.** Um sistema operacional completo, escrito em Pinker Next.

## Livros de referência

O projeto segue o passo a passo de três livros, em duas fases:

1. **Construir a Pinker:**
   - *Crafting Interpreters*, de Robert Nystrom.
   - *Writing an Interpreter in Go*, de Thorsten Ball.
2. **Usar a Pinker para construir o sistema** descrito em *The Elements of Computing
   Systems* (Nand2Tetris), de Noam Nisan e Shimon Schocken.

## Linguagens de implementação

Cada linguagem tem um papel central, definido por responsabilidade: usa-se cada uma
onde ela se sai melhor. Nem todas serão necessárias em toda tarefa.

- **Rust:** estrutura, frontend, representações e integrações.
- **C:** baixo nível, ABI, interfaces de máquina e pontos próximos do hardware.
- **Python:** ferramentas, geração, automação e trabalho auxiliar.
- **Bash:** cola entre as partes (scripts curtos de build e de setup). Terá uma
  importância muito alta mais adiante.

Exceções a essa divisão precisam ser especificadas.

### Sem dependências externas

Nenhuma das linguagens usa dependências ou bibliotecas externas. Externa é qualquer
biblioteca que não venha na instalação padrão da linguagem. As bibliotecas padrão (a
`std` do Rust, a libc do C, a biblioteca padrão do Python) são permitidas.

### Combinação entre linguagens

As linguagens não ficam isoladas artificialmente: os papéis se combinam quando há
razão técnica para isso.

### Fronteira Rust/C

Na fronteira entre Rust e C, estes pontos são contratos explícitos:

- **Layout** dos dados.
- **Alinhamento.**
- **Ownership:** quem aloca e quem libera cada recurso.
- **Ponteiros:** validade, nulidade e tempo de vida.
- **Erros:** como cada lado sinaliza uma falha.
- **Unwinding:** o que acontece quando um panic ou erro chega à fronteira.

## Educação: um pipeline transparente

Todo o pipeline da Pinker é pensado para ensinar. Um arquivo `<N>.pk` é, para o
sistema, só uma sequência de caracteres. Ele passa por várias etapas até chegar ao
estado final, em que a máquina o executa.

Quem tiver curiosidade deve poder ver o resultado de **qualquer** etapa do pipeline.
Para isso, a opção `--emit=<etapa>` grava o resultado dessa etapa em um arquivo
externo, que poderá ser inspecionado (por exemplo, `--emit=tokens` ou `--emit=ast`).
O `-o` mantém o significado convencional: o arquivo de saída final.

## Ferramentas de apoio

- **Âncoras de navegação.** Ver a seção abaixo.
- **`temp/`.** Diretório de trabalho descartável dos agentes; não sobe ao remoto.

## Âncoras de navegação

Os arquivos de código vão chegar a milhares de linhas. As âncoras de navegação marcam
trechos do código para que um agente encontre só o que procura, sem ler o arquivo
inteiro com `cat`. Quem lê as âncoras é o subcomando `pinker nav`, que busca em todo o
código-fonte.

Cada trecho fica entre duas âncoras, com uma breve descrição e palavras-chave:

```text
pink-nav: start
pink-resume: <o que o código faz>
pink-keywords: <palavra>, <palavra>, ...
... código ...
pink-nav: end
```

Todos os metadados usam o prefixo `pink-`. O par `pink-nav: start` / `pink-nav: end`
deixa claro que aquele trecho é para o `pinker nav`.

As âncoras ficam nos comentários da linguagem de cada arquivo (por exemplo, `//` em
Rust e C, `#` em Python e Bash), e o `pinker nav` segue a sintaxe de comentário de cada
uma. Os testes também levam âncoras.

### Hierarquia

Um bloco pode ficar dentro de outro, formando uma hierarquia de pais, filhos e netos
(por exemplo, uma função dentro da seção de um módulo).

### Busca exata

Cada bloco terá um identificador, e o `pinker nav` encontra exatamente o bloco pedido.
A busca é determinística: nada de indexador probabilístico.

### Saída do `pinker nav`

O `pinker nav` devolve o código do trecho, junto com os seus filhos e netos, e os
metadados Pinker (`pink-nav`, `pink-resume`, `pink-keywords`).

### `pinker nav check`

Uma checagem que aponta âncoras sem par ou sem `pink-resume`, para que âncoras
desatualizadas não levem o agente a erro.

### `pinker map` (futuro)

Um mapa de todas as âncoras, indentado segundo a hierarquia, com o resumo de cada bloco:

```text
pink-resume: A função a seguir chama X, que faz N...
    pink-resume: A função, ao chamar X, recebe os valores Y e...
        ...
```

### Regra operacional

O instrumento não pode ser maior que a sua razão de existir: entregar ao agente só o
código de que ele precisa. Cada recurso novo das âncoras precisa servir a esse fim.

O design evolui junto com a Pinker. Ainda faltam, entre outras coisas, os nomes de
seção e a identificação numérica.

## Em aberto

- As etapas do pipeline e o formato do que cada uma produz.
- Os nomes das etapas aceitos por `--emit`.
- Como as etapas de *The Elements of Computing Systems* (portas lógicas, CPU, memória,
  assembler, máquina virtual, compilador, sistema operacional) se distribuem entre a
  Pinker e a Pinker Next.
- Detalhes das âncoras de navegação: formato do identificador, nomes de seção e
  identificação numérica.
