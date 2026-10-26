# Visão da Pinker

> "O poder do C, a segurança do Rust e a simplicidade do Python."

## O que é a Pinker

A Pinker é uma linguagem de programação **estática** com sintaxe inspirada em Python.
Ela oferece recursos comuns às linguagens de alto nível, mas suas principais
capacidades estão no **baixo nível**.

## Para que ela existe

A Pinker é a base de um projeto maior, construído em camadas:

1. **O computador.** Antes do sistema operacional, construiremos a própria máquina:
   memória, armazenamento, tudo.
2. **Pinker Next.** Uma segunda linguagem, **dinâmica**, escrita em Pinker e mais
   próxima de Python ou Ruby do que de C. Ela herda a filosofia da Pinker.
3. **O sistema operacional.** Um sistema operacional completo, escrito em Pinker Next.

## Linguagens de implementação

Cada linguagem tem um papel central. Nem todas serão necessárias em toda tarefa: usa-se
a que se sai melhor naquilo.

- **Rust:** o código principal da Pinker.
- **C:** os assuntos de baixo nível.
- **Python:** a parte operacional, mecânica e de ferramentas.
- **Bash:** cola entre as partes (scripts curtos de build e de setup).

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

- Quais componentes ficam em C (por exemplo, o computador emulado ou o runtime).
- As etapas do pipeline e o formato do que cada uma produz.
- Os nomes das etapas aceitos por `--emit`.
- Como o "computador" será construído: emulador em software, hardware descrito em
  outra linguagem, ou outra abordagem.
- Detalhes das âncoras de navegação: formato do identificador, nomes de seção e
  identificação numérica.
