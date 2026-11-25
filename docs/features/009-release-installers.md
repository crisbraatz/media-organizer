# 009 — Instalação simplificada

O README abre com instruções rápidas para Linux, macOS e Windows. A instalação manual com SHA-256 continua disponível em uma seção recolhível.

Os instaladores escolhem a última release estável, identificam o pacote para o sistema e verificam o SHA-256 antes de extrair o executável. O mesmo comando instala ou atualiza para o usuário atual. A versão do executável é conferida antes de substituir uma instalação existente. No Windows, o instalador também tenta adicionar a pasta ao PATH do usuário.

## Verificação

A instalação e a reinstalação da versão pública 1.0.0 passaram no macOS Apple Silicon com um destino temporário. Os testes do instalador PowerShell passaram com pacotes sintéticos. Também confirmei o download, a verificação e a extração do pacote Windows público pelo PowerShell no macOS. A execução do binário e a alteração do PATH no Windows não foram testadas localmente. A CI inclui testes dos instaladores.
