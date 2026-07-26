# Letras Sync

**Letras Sync** é um Sistema de Projeção Inteligente de Letras Sincronizadas. O projeto é uma aplicação desktop desenvolvida para receber um link de uma música do YouTube, extrair automaticamente o áudio e a letra, e projetar o texto em um segundo monitor (projetor) perfeitamente sincronizado com a reprodução da música.

## 🎯 O que ele resolve?

O objetivo principal é minimizar a intervenção de um operador na projeção de letras (como em igrejas ou eventos). O sistema automatiza o fluxo: busca os metadados, baixa a música, obtém a legenda oficial ou utiliza Inteligência Artificial (Whisper) para gerar uma letra sincronizada caso não exista.

## 🛠 Como Funciona (Fluxo Principal)

1. O usuário cola a URL do vídeo do YouTube.
2. O sistema extrai o áudio via yt-dlp e busca por letras disponíveis através de Providers (YouTube Captions e LRCLib).
3. Se não encontrar letras, aciona o **faster-whisper** como fallback para ouvir o áudio e gerar a letra sincronizada.
4. O áudio é reproduzido pelo libmpv enquanto a letra é projetada em tela cheia no segundo monitor (texto amarelo em fundo preto via QML).

## 💻 Stack Tecnológico (MVP)

- **Linguagem:** Rust (Runtime Assíncrona: Tokio)
- **Interface (UI):** Qt 6 + Qt Quick (QML)
- **Player de Mídia:** libmpv
- **Processamento de Mídia:** FFmpeg
- **Download/Metadados YouTube:** yt-dlp
- **Busca de Letras:** YouTube Captions + LRCLib (Providers)
- **Fallback IA:** faster-whisper (Medium)
- **Banco de Dados:** SQLite (via sqlx)
- **Logs:** tracing + tracing-subscriber
- **Arquitetura:** Clean Architecture / Modular Architecture
