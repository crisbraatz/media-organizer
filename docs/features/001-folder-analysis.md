# 001 — Análise de pasta e relatório

Implementada em 24 de novembro de 2026 com Codex.

## Como usar

Execute `media-organizer` e informe o caminho quando solicitado. Também é possível passar a pasta direto ao comando, por exemplo `media-organizer "/caminho/para/fotos"`.

## O que o relatório mostra

A análise percorre todos os níveis de subpastas e mostra as quantidades de fotos, vídeos, duplicatas e subpastas. Os totais de mídias e duplicatas também aparecem por extensão. Se três arquivos tiverem conteúdo idêntico, o relatório conta duas duplicatas e atribui cada cópia à própria extensão. Uma pasta vazia gera totais iguais a zero. Se a análise não puder ler uma pasta ou arquivo necessário, ela informa o caminho e o erro.

## Verificação e segurança

A compilação, a formatação, o Clippy e os 11 testes passaram no macOS. Gitleaks e Semgrep não apontaram problemas nas alterações. O comando lê arquivos e preserva os nomes, o conteúdo e a estrutura das pastas. A busca ignora links encontrados dentro da pasta e, no Windows, pontos de reanálise, como junções e alguns arquivos de nuvem ainda não baixados.
