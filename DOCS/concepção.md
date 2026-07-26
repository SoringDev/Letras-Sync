# Letras Sync – Sistema de Projeção Inteligente de Letras Sincronizadas

# 1. Visão Geral

Desenvolver uma aplicação desktop multiplataforma (desenvolvimento em Fedora Linux e execução no Windows 11) capaz de reproduzir músicas a partir de um link do YouTube, obter automaticamente a letra sincronizada e exibi-la em tela cheia em um monitor secundário (projetor), mantendo sincronização precisa com a reprodução do áudio.

O sistema deverá minimizar a intervenção do operador, automatizando desde a obtenção da música até a exibição da letra.

---

# 2. Objetivos do MVP

Implementar uma solução funcional capaz de:

- Receber um link do YouTube.
- Obter metadados da música.
- Extrair ou gerar letra sincronizada.
- Reproduzir o áudio.
- Exibir a letra sincronizada em tela cheia.
- Projetar exclusivamente no segundo monitor.
- Operar com baixa latência e alta estabilidade.

---

# 3. Requisitos Funcionais

### Entrada

- Inserção de URL do YouTube.
- Botão "Carregar Música".

### Processamento

- Obter informações do vídeo.
- Extrair legendas sincronizadas quando disponíveis.
- Caso não existam legendas, gerar sincronização utilizando Whisper.
- Converter qualquer formato para um modelo interno padronizado.

### Reprodução

- Reprodução contínua do áudio.
- Controle de:
    - Play
    - Pause
    - Stop
    - Seek

### Exibição

- Fundo preto.
- Texto amarelo puro (#FFFF00).
- Fonte escalável.
- Centralização automática.
- Atualização sincronizada com o áudio.
- Exibição apenas no monitor do projetor.

---

# 4. Requisitos Não Funcionais

- Compatibilidade com Windows 11.
- Desenvolvimento realizado em Fedora Linux.
- Interface responsiva.
- Arquitetura modular.
- Baixo consumo de memória.
- Cache local.
- Funcionamento offline para músicas previamente processadas.

---

# 5. Arquitetura (Macro)

A arquitetura do sistema adota os seguintes padrões:

- Clean Architecture
- Repository Pattern
- Provider Pattern (Lyrics Providers)
- Event Bus
- State Machine (Player)
- Modular Architecture

---

# 6. Módulos

## 6.1 UI

Responsável por:

- Interface do operador
- Configurações
- Playlist
- Monitoramento

Não contém lógica de negócio.

---

## 6.2 Youtube Service

Responsável por:

- Download de metadados
- Download de legendas
- Download de áudio (quando permitido)
- Thumbnail
- Título
- Autor

Saída:

```
MusicMetadata
```

---

## 6.3 Lyrics Engine

Responsável por:

- Interpretar SRT
- Interpretar LRC
- Interpretar VTT
- Receber saída do Whisper
- Converter tudo para o modelo interno

Modelo:

```
{
  "start":12.50,
  "end":15.30,
  "text":"Grande é o Senhor"
}
```

---

## 6.4 Timeline Engine

Responsável por:

- Controlar sincronização
- Determinar linha atual
- Atualizar interface
- Gerenciar eventos de mudança

É o núcleo do sistema.

---

## 6.5 Audio Engine

Responsável por:

- Reprodução
- Controle de tempo
- Eventos
- Estado do player

---

## 6.6 Database

Persistência de:

- Histórico
- Letras
- Configurações
- Cache
- Arquivos sincronizados

---

# 7. Fluxo Principal

```
URL YouTube
      │
      ▼
yt-dlp
      │
      ├── Metadados
      ├── Áudio
      └── Legendas
               │
               ▼
Lyrics Providers
    ├── Cache
    ├── YouTube
    ├── LRCLib
    └── faster-whisper (fallback)
               │
               ▼
Timeline / Synchronization Engine
               │
               ▼
libmpv
               │
               ▼
Projection Renderer (QML)
```

---

# 8. Estrutura do Projeto

```
presentation/
    ui/
    projection/

application/
    player/
    timeline/
    playlist/

domain/
    music/
    lyrics/
    settings/

infrastructure/
    youtube/
    providers/
    database/
    audio/
    cache/
    logging/

shared/
    models/
    config/
    utils/
```

---

# 9. Modelo de Dados

## Music

```
id

title

artist

youtube_url

duration

thumbnail

created_at
```

---

## LyricsLine

```
id

music_id

start_time

end_time

text
```

---

## Settings

```
font_size

font_family

font_color

background_color

projector_monitor

cache_path
```

---

# 10. Tecnologias

- **Linguagem:** Rust
- **UI:** Qt 6 + Qt Quick (QML)
- **Player de mídia:** libmpv
- **Processamento de mídia:** FFmpeg
- **Runtime assíncrona:** Tokio
- **Banco de dados:** SQLite
- **Acesso ao banco:** sqlx
- **Serialização:** serde
- **Configuração:** TOML
- **Download/Metadados YouTube:** yt-dlp
- **Busca de letras:** YouTube Captions + LRCLib (Providers)
- **Fallback IA:** faster-whisper (Medium)
- **Logs:** tracing + tracing-subscriber
- **Comunicação entre módulos:** tokio::sync (ou crossbeam-channel)
- **Gerenciamento de diretórios:** directories
- **Build:** Cargo
- **Versionamento:** Git

---

# 11. Organização da Aplicação

Cada módulo deve possuir:

```
Interface Pública

↓

Serviço

↓

Modelos

↓

Implementação
```

Dependências entre módulos devem ocorrer apenas através de interfaces bem definidas.

---

# 12. Estratégia de Sincronização

Todos os formatos de legenda serão convertidos para um único modelo interno.

```
SRT

↓

LRC

↓

VTT

↓

Whisper

↓

TimelineModel
```

Isso elimina tratamentos específicos durante a renderização.

---

# 13. Interface Inicial (MVP)

Tela principal contendo:

- Campo URL
- Botão Carregar
- Informações da música
- Botões Play/Pause/Stop
- Barra de progresso
- Botão "Projetar"

Ao iniciar a projeção:

- Tela cheia no monitor secundário
- Fundo preto
- Texto amarelo
- Sincronização automática

---

# 14. Critérios de Aceitação

O MVP será considerado concluído quando for possível:

- Carregar uma música por URL.
- Obter automaticamente letra sincronizada ou gerá-la.
- Reproduzir áudio sem interrupções.
- Exibir a letra sincronizada em tela cheia.
- Projetar corretamente no segundo monitor.
- Funcionar integralmente no Windows 11.

---

# 15. Roadmap

## Fase 1 — Infraestrutura

- Estrutura do projeto
- Sistema de módulos
- Configurações
- Banco SQLite
- Logging

---

## Fase 2 — Reprodução

- Integração com libmpv
- Controle de reprodução
- Barra de progresso

---

## Fase 3 — Integração com YouTube

- Metadados
- Extração de legendas
- Cache local

---

## Fase 4 — Motor de Letras

- Parser LRC
- Parser SRT
- Parser VTT
- Modelo unificado

---

## Fase 5 — Timeline

- Sincronização baseada em tempo
- Atualização automática
- Eventos de troca de linha

---

## Fase 6 — Projeção

- Segundo monitor
- Tela cheia
- Renderização otimizada
- Alto contraste

---

## Fase 7 — Fallback com IA

- Integração com Whisper
- Geração automática de sincronização
- Armazenamento do resultado em cache

---

# 16. Evoluções Futuras

- Editor visual de sincronização de letras.
- Destaque progressivo por palavra (efeito karaokê).
- Playlists e ordem de culto.
- Busca integrada por músicas.
- Controle remoto via navegador ou aplicativo móvel.
- Temas de acessibilidade (fontes, contraste e tamanhos).
- Suporte a múltiplos idiomas.
- Integração com bibliotecas locais de áudio e vídeo.
- Exportação/importação de letras em formatos LRC, SRT e VTT.
- Atualizações automáticas da aplicação.