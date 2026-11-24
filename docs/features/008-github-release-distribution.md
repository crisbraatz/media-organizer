# 008 — Distribuição da versão 1.0.0

A versão do executável é `1.0.0`. O workflow de release prepara pacotes para Linux x64, Windows x64, macOS Intel e macOS Apple Silicon. Cada pacote traz o executável, a licença MIT e os avisos de terceiros. O workflow extrai o executável em cada plataforma, confere `--version` e gera o arquivo `SHA256SUMS`.

A execução manual prepara e verifica os arquivos sem publicar uma release. A publicação requer uma tag igual à versão do executável e o conjunto completo de pacotes verificados.

O README segue a estrutura do AI Workflow e explica como instalar e verificar os downloads. O projeto inclui guias de contribuição, segurança e conduta, além de modelos de issue e pull request.

## Verificação

Os 57 testes passaram no macOS. O pacote Apple Silicon foi extraído e executado. A compilação estática para Linux x64 e a verificação de tipos para Windows x64 passaram. Os quatro conjuntos de avisos de licença foram gerados. A verificação de integridade rejeitou um arquivo adulterado. A matriz do GitHub ainda precisa executar os quatro alvos antes da publicação.
