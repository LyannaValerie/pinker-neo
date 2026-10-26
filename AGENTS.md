# Agentes na Pinker Neo

Ao iniciar ou retomar uma tarefa, carregue a skill
[`pinker-neo`](.claude/skills/pinker-neo/SKILL.md), mesmo que o seu cliente não a ative
sozinho: no Claude Code, pela ferramenta de skills; nos outros agentes, lendo o arquivo.
Ela dá a sequência de trabalho. As regras ficam em
[`docs/protocolo-agentes.md`](docs/protocolo-agentes.md).

Se você foi lançado como revisor, siga só o prompt da revisão: não carregue a sequência
do implementador, não lance outro revisor e não escreva nada.

O estado de cada tarefa fica em `.tarefas/<id>/`, fora do Git. A visão do projeto está em
[`docs/visao.md`](docs/visao.md).
