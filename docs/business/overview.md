# media-organizer — Visão do produto

## Visão

Ajudar pessoas a colocar seu acervo pessoal de fotos e vídeos em ordem cronológica com uma ferramenta de terminal. A nova versão será escrita do zero, em outra linguagem, a partir das definições de negócio e do comportamento descrito no README original.

## Problema e público

Fotos e vídeos de celulares, câmeras, downloads e exportações chegam com nomes diferentes. Isso dificulta percorrer o acervo na ordem em que os registros foram feitos. A ferramenta é voltada a quem organiza o próprio acervo.

## Proposta de valor

O comando analisa a pasta escolhida e apresenta um resumo antes de alterar arquivos. Após a confirmação da pessoa, renomeia as mídias em sequência pela data de captura, usando a data de criação do arquivo quando a data de captura não consta nos metadados. A pessoa pode optar por remover duplicatas detectadas pelo conteúdo e reunir mídias de subpastas na pasta principal.

## Modelo e papéis

Ferramenta gratuita e de código aberto. A pessoa que opera o comando escolhe a pasta, revisa a análise e autoriza as alterações.

## Canal e plataformas

Comando interativo `media-organizer` no terminal, disponível para Linux, Windows e macOS.

## Dados e integrações

A ferramenta trabalha com fotos, vídeos e metadados do acervo pessoal. Processamento local, sem conta, envio de mídia ou integração com serviços externos durante a organização (premissa). A distribuição pelo GitHub Releases segue o modelo do original (premissa).

## Restrições e sucesso da primeira versão

A primeira versão estará pronta quando reproduzir o fluxo e as funcionalidades documentadas no README original nas três plataformas. A pessoa deve poder analisar uma pasta, decidir como tratar duplicatas e subpastas e confirmar a organização, obtendo a sequência cronológica com a numeração documentada.

[README do MediaOrganizerOld](https://github.com/crisbraatz/MediaOrganizerOld#readme)
