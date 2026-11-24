# 002 — Prévia da renomeação cronológica

Implementada em 24 de novembro de 2026 com Codex.

## Como usar

Execute `media-organizer --preview "/caminho/para/fotos"`. Se omitir o caminho, o comando solicita a pasta pelo terminal. A prévia aparece depois do relatório de análise.

## Como os nomes são propostos

Cada pasta recebe sua própria sequência, começando em 1. O total de mídias na pasta determina quantos dígitos usar, e os nomes mantêm as extensões originais. A ordenação usa a data de captura ou, quando ela não está disponível, a data de criação. Mídias sem nenhuma data aparecem no fim. O nome atual resolve empates.

Datas com fuso explícito são convertidas para UTC. Para comparar datas sem fuso, a prévia também as interpreta como UTC. A leitura de metadados foi testada com JPEG, DNG e MP4. A disponibilidade da data embutida depende do formato e do arquivo.

A marca `[blocked]` indica um destino ocupado por uma entrada que o plano não move. Essa indicação cobre conflitos diretos. A prévia preserva arquivos e pastas.

## Verificação

A compilação, a formatação, o Clippy e os 23 testes passaram no macOS. Gitleaks e Semgrep não encontraram problemas nas verificações executadas. A auditoria de dependências retornou um aviso informativo de que a biblioteca indireta `chrono` deixou de ser mantida. [Aviso do RustSec](https://rustsec.org/advisories/RUSTSEC-2026-0543.html)
