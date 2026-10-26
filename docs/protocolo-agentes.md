# Protocolo de trabalho dos agentes

Três mecanismos para que um trabalho de agente seja delimitado, retomável e revisado:

1. **Contrato da tarefa** (`task.json`): o que foi autorizado, o que fica de fora e como
   saber que terminou.
2. **Checkpoint** (`checkpoint.md`): o estado atual, escrito para que outro executor
   retome sem depender da memória de quem começou.
3. **Revisão adversarial independente**: outro agente, só em leitura, tenta derrubar a
   conclusão antes da entrega.

A tarefa pertence ao projeto; o executor pode ser trocado sem reiniciá-la. Cada etapa é
uma afirmação diferente, e nenhuma implica a seguinte:

```text
IMPLEMENTADO != TESTADO != REVISADO != AUTORIZADO PARA PUBLICAÇÃO
```

Este protocolo vale só para a Pinker Neo. As regras da Pinker v0 não são autoridade aqui.
A Founder é a pessoa responsável pelo projeto (a conta `LyannaValerie` no GitHub): é quem
autoriza as tarefas, e fala com os agentes pela conversa em que eles rodam.

## Onde fica cada coisa

| O quê | Onde | Git |
|---|---|---|
| Este protocolo | `docs/protocolo-agentes.md` | versionado |
| Modelos | `docs/modelos/task.json`, `docs/modelos/checkpoint.md` | versionados |
| Estado real de cada tarefa | `.tarefas/<id>/` | ignorado |

O diretório de uma tarefa guarda:

```text
.tarefas/<id>/
  task.json        o contrato, preenchido a partir do modelo
  checkpoint.md    o estado atual
  evidencias/      snapshots de candidatos, saídas de validação, relatórios de revisão
```

- `<id>` é estável e não muda com o executor: `AAAA-MM-DD-nome-curto`.
- `.tarefas/` fica fora de `target/` (apagado por `cargo clean`) e de `temp/` (trabalho
  descartável). Nunca rode `git clean -X` ou `git clean -x`: eles apagam arquivos
  ignorados, e o estado das tarefas está entre eles.
- O estado local permite retomar só na máquina e no usuário onde ele está. Não é backup
  nem transferência entre máquinas.
- Nada de segredos nem de transcrições de conversa nesses arquivos.

## Iniciar uma tarefa

1. Confirme a autorização: um item do cronograma ou um pedido direto da Founder. Sem
   cronograma no repositório, registre o pedido direto; não invente um roadmap.
2. Observe o repositório: branch, HEAD e `git status`. Trabalho não commitado que já
   existia não é seu: preserve-o e registre-o no checkpoint.
3. Crie `.tarefas/<id>/`, copie os dois modelos e preencha-os.

## Atualizar o checkpoint

Atualize nos pontos materiais: antes de um trabalho longo, depois de uma alteração
coerente, depois de validar, depois de uma revisão ou correção, ao encontrar um bloqueio
e antes de transferir ou encerrar. Não conte com conseguir escrever um resumo quando a
quota acabar.

- Reescreva o estado atual; não acumule um diário. Guarde da história só o que muda uma
  decisão de quem retoma.
- Escreva a versão nova inteira em `checkpoint.md.novo` e só então troque:
  `mv checkpoint.md.novo checkpoint.md`. Um checkpoint interrompido no meio nunca
  substitui o anterior. O `task.json` é atualizado do mesmo jeito (`task.json.novo`).
- Ao parar, transferir ou encerrar, ponha `executor_atual` em `null` no `task.json`.
  `null` é uma **parada declarada**: diz que o executor anunciou que parou, não prova que
  nada mais está modificando a tarefa. Enquanto ele tiver um nome, o executor nomeado
  pode estar ativo, mesmo esperando uma revisão.

## Evidências: o que foi avaliado

Cada validação e cada revisão nomeiam o **candidato** que avaliaram:

- **Commitado:** o SHA.
- **Não commitado:** um snapshot em `evidencias/<candidato>/`, com:
  - o diff dos arquivos alterados contra o HEAD, `git diff HEAD -- <arquivos>` (o `git
    diff` sem `HEAD` não mostra o que já está no índice);
  - uma cópia dos arquivos novos;
  - o `git hash-object <arquivos>` de todos os arquivos do candidato, para conferir os
    arquivos vivos (e o commit, depois) contra os bytes avaliados;
  - um `sha256sum` do conteúdo do snapshot, com caminhos relativos ao próprio snapshot.

  O HEAD não identifica bytes que ainda não foram commitados.

Código alterado depois de uma validação torna essa evidência histórica: revalide o
comportamento afetado com o candidato novo. Registre só comandos realmente executados,
com o resultado que eles deram.

## Retomar ou transferir

1. Confirme que nenhum executor continua modificando a tarefa. Essa é uma condição a
   confirmar, separada da parada declarada: quem confirma é a Founder, com ou sem
   `executor_atual` em `null`. O executor anterior só pode confirmar a própria parada.
   Sem resposta, a ausência não está provada: não comece a escrever em paralelo.
2. Leia as instruções do repositório, o `task.json` e o `checkpoint.md`.
3. Observe de novo `git status`, HEAD e o diff, e compare com o checkpoint. Se não
   baterem, registre a diferença antes de continuar.
4. Mantenha a tarefa, o branch e o diretório de trabalho. Atualize `executor_atual` no
   `task.json` e retome da próxima ação do checkpoint.

Nunca use `reset`, `clean`, exclusão ou um checkout novo para esconder uma divergência
ou descartar trabalho não commitado. Se o usuário do sistema mudou e faltar acesso a
algum arquivo, pare e peça à Founder: nada de `chmod` amplo, `sudo` ou credenciais
emprestadas.

## Revisão adversarial

Uma revisão por entrega coerente, depois da validação.

1. Registre o checkpoint e o snapshot do candidato.
2. Entregue ao revisor, num contexto novo, o objetivo, o escopo, os critérios, o
   candidato e as evidências. O revisor só lê.
3. O revisor procura defeitos concretos e tenta falsificar a conclusão, nesta ordem:
   comportamento incorreto, regressão, teste que passa sem provar o que deveria, perda
   de trabalho numa retomada, contradição operacional e ampliação do escopo sem
   autorização. Preferência estética e funcionalidade futura não bloqueiam a entrega.
4. Cada achado traz a localização, a consequência, uma reprodução ou um argumento
   verificável, e a gravidade justificada.
5. O implementador classifica cada achado: **confirmado** (corrigir), **não
   confirmado** (justificar com evidência) ou **decisão da Founder** (encaminhar).
6. Depois das correções, uma revisão dirigida aos achados e ao efeito das mudanças. Uma
   nova revisão ampla só se o escopo aumentou.

### Rota do revisor

O revisor é o Codex pelo `codex-velina`, com o modelo `gpt-5.6-terra` (Terra 5.6),
esforço `high`, sessão efêmera (`--ephemeral`) e sandbox `read-only`. O comando exato
está na skill [`pinker-neo`](../.claude/skills/pinker-neo/SKILL.md). Não troque o revisor
por um subagente Claude ou por outro modelo.

- O implementador lança o revisor, sem pedir nova autorização à Founder, e recebe a
  resposta. O prompt segue [`modelos/prompt-revisao.md`](modelos/prompt-revisao.md): sem
  conclusão esperada e sem induzir aprovação.
- Cada rodada é uma sessão nova. A seguinte recebe o candidato novo, os achados
  anteriores, as decisões e as correções, e se concentra em fechar os achados e encontrar
  regressões das correções, sem refazer a auditoria ampla.
- Efêmera quer dizer sem continuidade implícita do revisor, não sem registro: o prompt,
  o comando, a saída, o código de término e o relatório ficam em
  `.tarefas/<id>/evidencias/`, sem credenciais.
- Os arquivos do candidato ficam parados durante a revisão. Compare os hashes antes e
  depois para mostrar que o revisor não os alterou.

### Convergência

O ciclo de revisão termina quando os critérios da tarefa estão cumpridos, as validações
passam, não sobra achado bloqueante confirmado, as correções foram conferidas no
candidato final e as sugestões não bloqueantes estão registradas. Não é preciso esgotar
todas as sugestões possíveis, e um número de rodadas não basta para aceitar.

Pare o ciclo e leve à Founder a menor decisão necessária quando houver repetição sem
evidência nova, disputa sobre o contrato, ampliação de escopo ou correções seguidas que
não convergem. Registre o ponto exato no checkpoint.

Se faltar quota ou a ferramenta falhar, preserve a tarefa e o checkpoint e marque a
revisão como pendente. Não troque de revisor, não desligue as restrições e não declare a
entrega revisada. Uma autorrevisão nunca é apresentada como revisão independente.

## Escopo

O cronograma diz a ordem planejada; a autorização diz o que pode ser feito. Terminar uma
etapa não autoriza começar a seguinte. Durante a tarefa:

| Encontrei | Faço |
|---|---|
| Defeito introduzido pela própria alteração | Corrijo nesta tarefa. |
| Dependência indispensável, dentro do escopo | Registro e resolvo no menor recorte. |
| Dependência que muda a linguagem ou a arquitetura | Levo à Founder a menor decisão necessária. |
| Melhoria independente | Registro em uma linha, como fora do escopo, e sigo. |

Não abra uma issue para cada achado, e não comece uma tarefa adiada só porque o arquivo
já está aberto.

## Concluir

A tarefa termina quando os critérios do `task.json` estão cumpridos, a revisão do
candidato final está registrada e o trabalho fora do escopo tem destino definido. Commit
local só dos arquivos da tarefa. Push, merge e release exigem autorização própria.
