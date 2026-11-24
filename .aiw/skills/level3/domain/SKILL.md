---
name: domain
description: Load when changing personal media organization workflows or rules.
level: 3
updated: 2026-11-24
---

# Organização de acervos pessoais

## Glossário

- **Pasta principal**. Pasta escolhida para a organização.
- **Duplicata**. Mídia com conteúdo idêntico ao de outra.
- **Data de captura**. Data indicada nos metadados da mídia.

## Regras de negócio

1. A ordenação usa a data de captura, com a data de criação do arquivo como alternativa.
2. A sequência começa em 1. Complete com zeros à esquerda conforme o número de dígitos da contagem final e preserve a extensão.
3. A remoção de duplicatas e a reunião de arquivos na pasta principal são opcionais. Quando escolhidas, precedem a renomeação final.
4. A análise inclui todos os níveis de subpastas. Cada grupo de arquivos com conteúdo idêntico conta como uma mídia original e uma duplicata por cópia adicional. A contagem é atribuída ao tipo e à extensão de cada cópia (funcionalidade 001).

5. A prévia calcula uma sequência por pasta. Datas indisponíveis ficam no fim, empates seguem o nome atual e a extensão é preservada. A prévia indica destinos ocupados por entradas que ficam no lugar (funcionalidade 002).

## Invariantes

- A análise é apresentada antes de qualquer alteração.
- A pessoa confirma as opções antes de aplicar mudanças.

## Fluxo

A pessoa escolhe uma pasta, revisa a análise, escolhe como tratar duplicatas e subpastas e confirma a organização.

[README original](https://github.com/crisbraatz/MediaOrganizerOld#readme)
