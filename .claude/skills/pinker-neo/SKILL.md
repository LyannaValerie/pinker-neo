---
name: pinker-neo
description: Procedimento de trabalho na Pinker Neo — escopo, checkpoint, ferramentas, validação, revisão adversarial pelo codex-velina e relatório. Carregue ao iniciar ou retomar qualquer tarefa neste repositório.
---
# Pinker Neo — procedimento de trabalho

As regras estão em [`docs/protocolo-agentes.md`](../../../docs/protocolo-agentes.md); esta
skill é a sequência. Se as duas divergirem, vale o protocolo. Leia de cada documento só a
seção de que a tarefa precisa.

## Sequência

1. **Observe:** `git status -sb`, HEAD, trabalho não commitado e as tarefas em
   `.tarefas/`. O que já existia não é seu: preserve.
2. **Tarefa:** crie `.tarefas/<id>/` a partir de `docs/modelos/task.json` e
   `docs/modelos/checkpoint.md`, ou retome uma existente (protocolo, "Retomar ou
   transferir").
3. **Escopo:** só o que a autorização cobre; o resto segue a tabela "Escopo" do protocolo.
4. **Ferramentas:**
   - Navegação: ainda não há `pinker nav`. Confira com `cargo run -q -- --help`; se ele
     continuar ausente, use `grep -n "pink-nav\|pink-resume\|pink-keywords" <arquivo>` e
     leia só o trecho encontrado. As âncoras no código não provam que o navegador existe.
   - Book: ainda não integrado. Não o instale, e a ausência dele não bloqueia nada.
   - Nada da Pinker v0 vale aqui por inferência.
5. **Implemente**, atualizando o checkpoint nos pontos materiais.
6. **Valide** na proporção da mudança e registre cada comando literal, o código de saída
   e o candidato:
   - código: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
     `cargo test`;
   - só documentos: `git diff --check`, JSON válido, referências existentes.
7. **Revise:** com o candidato identificado e parado, escreva o prompt a partir de
   [`docs/modelos/prompt-revisao.md`](../../../docs/modelos/prompt-revisao.md) e lance o
   revisor (abaixo), sem pedir autorização à Founder.
8. **Trate os achados:** confirmado → corrija; improcedente → justifique com evidência;
   mudança de direção ou de escopo → Founder. Revalide, identifique o candidato novo e
   faça uma rodada nova, dirigida aos achados, até convergir (protocolo, "Convergência").
9. **Conclua:** checkpoint final, commit local só dos arquivos da tarefa e relatório.
   Push só com autorização.

## Revisor

```sh
E=.tarefas/<id>/evidencias N=1   # N = número da rodada
codex-velina exec -m gpt-5.6-terra -c 'model_reasoning_effort="high"' \
  -s read-only --ephemeral -C /home/vicky/pinker --color never --json \
  -o "$E/revisao-$N.md" - < "$E/prompt-revisao-$N.md" \
  > "$E/revisao-$N.jsonl" 2> "$E/revisao-$N.stderr"
echo $? > "$E/revisao-$N.codigo"
```

- Antes e depois, grave `git status --short` e `git hash-object` dos arquivos do
  candidato, e confira que não mudaram.
- Confira no `revisao-$N.jsonl` o modelo e o esforço que a sessão de fato usou.
- O relatório final do revisor fica em `revisao-$N.md`. O código diferente de 0, quota
  esgotada ou falta de login (`! codex-velina login`) deixam a revisão pendente. Não
  troque o revisor.

## Interrompa e leve à Founder

- Uma decisão de direção, de arquitetura ou de escopo.
- Um ciclo de revisão que não converge (protocolo, "Convergência").
- Dúvida sobre outro executor ativo.

## Concluído quando

Os critérios do `task.json` estão cumpridos, as validações passam, nenhum achado
bloqueante confirmado está pendente, as correções foram conferidas no candidato final
e o checkpoint registra o estado final com `executor_atual: null`.
