# 004 — Remoção opcional de duplicatas

## Como usar

- `media-organizer --preview --remove-duplicates "/caminho/para/fotos"` mostra quais cópias seriam removidas e quais arquivos ficariam.
- `media-organizer --apply --remove-duplicates "/caminho/para/fotos"` mostra o plano, avisa que a remoção é permanente e aguarda `yes` seguido de Enter.

O comando compara o conteúdo byte a byte, inclusive entre subpastas e extensões diferentes. Em cada grupo, mantém o primeiro arquivo em ordem de caminho. A numeração de cada pasta considera apenas as mídias mantidas.

## Exclusão e recuperação

O comando guarda as cópias sob nomes temporários durante a renomeação. Só depois de concluir todas as renomeações, verifica novamente cada cópia e o arquivo mantido antes de apagar a cópia. Se a renomeação falhar, tenta restaurar os nomes originais. Se a exclusão falhar, interrompe a limpeza e lista as cópias já apagadas e os últimos caminhos conhecidos das restantes. Nessa etapa, os nomes novos das mídias mantidas ficam no lugar.

## Verificação

Os 45 testes passaram no macOS, incluindo cancelamento, exclusão entre subpastas e preservação das cópias quando a renomeação falha. A verificação de compilação para Windows x64 passou. Um teste em container Linux ARM64 removeu uma cópia sintética entre subpastas e preservou o conteúdo das duas mídias mantidas. Gitleaks e Semgrep não apontaram problemas nas alterações. Os testes usam arquivos temporários.
