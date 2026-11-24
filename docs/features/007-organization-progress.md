# 007 — Progresso da organização no terminal

Depois da confirmação, o `media-organizer` mostra a etapa atual e sua contagem. A barra acompanha a preparação, a renomeação, a exclusão de duplicatas e a verificação de subpastas, conforme o plano. Se uma movimentação falhar, ela conta as tentativas de restauração. O resumo indica o resultado e eventuais caminhos que exigem recuperação manual.

A análise, a prévia e a saída redirecionada mantêm o formato anterior. Se o terminal falhar ao desenhar a barra, a operação e a recuperação continuam.

## Verificação

Os 57 testes passaram no macOS. Um teste em pseudoterminal verificou as etapas, a limpeza da linha antes do resumo e a saída redirecionada. A verificação de compilação para Windows x64 passou. Gitleaks e Semgrep não encontraram problemas no código.
