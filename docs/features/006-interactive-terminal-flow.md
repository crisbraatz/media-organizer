# 006 — Modo interativo no terminal

## Como usar

Abra `media-organizer` sem argumentos em um terminal. Digite o caminho e use Tab para completar nomes de pastas. Quando vários nomes correspondem, Tab completa o prefixo comum.

Depois do relatório, use as setas e Enter para escolher se quer remover duplicatas e reunir subpastas. As duas opções começam desativadas. Na prévia, escolha sair, recomeçar ou aplicar. Sair é a opção inicial. Para aplicar, digite `yes` seguido de Enter. O comando avisa antes se houver exclusão permanente de cópias.

Os comandos com argumentos e as sessões com entrada ou saída redirecionada seguem o fluxo anterior.

## Verificação

Os 55 testes passaram no macOS. Um teste em pseudoterminal verificou as teclas, o reinício, a restauração do terminal antes da confirmação e o cancelamento sem alterar os arquivos. No Linux ARM64, o container compilou e o fluxo interativo permitiu completar um caminho, aceitar as opções padrão e sair. A verificação de compilação para Windows x64 passou. A auditoria de dependências apresentou apenas o aviso informativo já conhecido sobre `chrono`. [Aviso do RustSec](https://rustsec.org/advisories/RUSTSEC-2026-0543.html)
