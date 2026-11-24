# 003 — Aplicar a renomeação cronológica

Implementada em 24 de novembro de 2026 com Codex.

## Como usar

Execute `media-organizer --apply "/caminho/para/fotos"`, revise os nomes propostos e digite `yes` seguido de Enter para aplicar. Qualquer outra resposta, inclusive o fim da entrada, cancela a operação. O comando verifica novamente o plano depois da confirmação.

A renomeação mantém cada mídia em sua pasta e preserva conteúdo e extensão. Nomes temporários permitem trocas entre arquivos do plano. Destinos ocupados por entradas que ficam no lugar bloqueiam a operação antes da primeira mudança. As operações do sistema de arquivos preservam destinos que apareçam depois da verificação.

Se uma operação falhar, o comando tenta restaurar os nomes originais e relata os casos que exigem recuperação manual. Uma interrupção do processo pode deixar arquivos com nomes temporários `.media-organizer-*.stage` ou com nomes finais. A verificação usa o estado atual do acervo e recomenda-se evitar alterações concorrentes durante a execução.

## Verificação e segurança

Os 37 testes, o Clippy e a compilação de release passaram no macOS. A compilação do código e dos testes foi verificada para Windows x64. No container Linux ARM64, a renomeação de uma amostra preservou seu conteúdo. Gitleaks e Semgrep não encontraram problemas. O OSV registrou o aviso informativo já conhecido sobre a manutenção de `chrono`. [Aviso do RustSec](https://rustsec.org/advisories/RUSTSEC-2026-0543.html)
