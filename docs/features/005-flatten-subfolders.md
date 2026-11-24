# 005 — Reunir mídias na pasta principal

## Como usar

- `media-organizer --preview --flatten "/caminho/para/fotos"` exibe os caminhos atuais e os novos nomes.
- `media-organizer --apply --flatten "/caminho/para/fotos"` exibe o plano e pede `yes` seguido de Enter para aplicar.
- Acrescente `--remove-duplicates` a qualquer um dos dois comandos para calcular a sequência apenas com as mídias mantidas.

As mídias recebem uma única sequência cronológica na pasta principal. A aplicação verifica o plano novamente, preserva destinos ocupados e tenta devolver os arquivos aos caminhos originais se uma movimentação falhar. A movimentação entre sistemas de arquivos diferentes pode falhar e acionar essa recuperação.

Ao concluir as mudanças nos arquivos, o comando tenta remover somente as subpastas afetadas que ficaram vazias, começando pelas mais profundas. Pastas vazias que não foram afetadas e pastas com outros arquivos ficam no lugar. Se a limpeza falhar, o comando relata o erro e mantém as mudanças de mídia concluídas.

## Verificação

Os 51 testes passaram no macOS, incluindo restauração após colisão durante a execução. A verificação de compilação para Windows x64 passou. No container Linux ARM64, o fluxo combinado preservou o conteúdo das mídias mantidas e o arquivo que não era mídia. Gitleaks e Semgrep não apontaram problemas nas alterações.
