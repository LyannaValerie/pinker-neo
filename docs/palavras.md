# Vocabulário da Pinker

Palavras reservadas, operadores por palavra, literais, tipos e intrínsecas propostos
para a Pinker.

| Palavra | Para que serve | Categoria |
|---|---|---|
| `package` | Declarar o pacote ao qual o código pertence | Palavra reservada |
| `bring` | Importar nomes de outro pacote ou módulo | Palavra reservada |
| `as` | Dar um nome local a uma importação | Palavra reservada |
| `public` | Tornar uma declaração acessível externamente | Palavra reservada |
| `fun` | Declarar uma função | Palavra reservada |
| `give` | Retornar um resultado e encerrar a função | Palavra reservada |
| `let` | Declarar uma variável | Palavra reservada |
| `mut` | Indicar mutabilidade | Palavra reservada |
| `fixed` | Declarar uma constante | Palavra reservada |
| `shape` | Declarar uma estrutura com campos nomeados | Palavra reservada |
| `enum` | Declarar um tipo composto por variantes nomeadas | Palavra reservada |
| `union` | Declarar um tipo que admite diferentes tipos constituintes | Palavra reservada |
| `alias` | Dar outro nome a um tipo | Palavra reservada |
| `pact` | Declarar um contrato de operações | Palavra reservada |
| `impl` | Implementar operações de um tipo ou cumprir um contrato | Palavra reservada |
| `self` | Referir-se à instância que recebe uma operação | Palavra reservada |
| `if` | Executar um caminho quando uma condição for verdadeira | Palavra reservada |
| `else` | Executar o caminho alternativo | Palavra reservada |
| `while` | Repetir enquanto uma condição for verdadeira | Palavra reservada |
| `for` | Percorrer os elementos de uma sequência | Palavra reservada |
| `in` | Relacionar a variável de iteração à sequência percorrida | Palavra reservada |
| `loop` | Repetir até uma interrupção explícita | Palavra reservada |
| `break` | Encerrar o laço atual | Palavra reservada |
| `continue` | Passar à próxima iteração | Palavra reservada |
| `match` | Selecionar um caminho conforme o padrão de um valor | Palavra reservada |
| `when` | Acrescentar uma condição a um padrão | Palavra reservada |
| `try` | Iniciar uma construção de tratamento de resultados falíveis | Palavra reservada |
| `propagate` | Encaminhar uma falha ao chamador | Palavra reservada |
| `and` | Conjunção lógica com curto-circuito | Operador por palavra |
| `or` | Disjunção lógica com curto-circuito | Operador por palavra |
| `not` | Negação lógica | Operador por palavra |
| `true` | Valor lógico verdadeiro | Literal reservado |
| `false` | Valor lógico falso | Literal reservado |
| `int` | Inteiro padrão; proposta: apelido de `i64` (ver [`decisoes.md`](decisoes.md)) | Tipo |
| `i8`, `i16`, `i32`, `i64` | Inteiros com sinal, de largura explícita | Tipo |
| `u8`, `u16`, `u32`, `u64` | Inteiros sem sinal, de largura explícita | Tipo |
| `bool` | Representar valores lógicos | Tipo |
| `text` | Representar texto | Tipo |
| `list` | Representar uma coleção sequencial dinâmica | Tipo |
| `map` | Associar chaves a valores | Tipo |
| `ptr` | Representar um ponteiro explícito | Tipo |
| `Result` | Representar sucesso ou falha de uma operação | Tipo |
| `say` | Escrever uma saída textual | Intrínseca ou função |
| `morph` | Converter um valor conforme as conversões permitidas | Intrínseca ou operação |
| `asm` | Inserir assembly explícito | Intrínseca |
| `alloc` | Alocar uma região de memória | Intrínseca ou função |
| `free` | Liberar uma região de memória | Intrínseca ou função |
