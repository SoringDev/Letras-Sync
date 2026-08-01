# Letras Sync

Aplicação desktop multiplataforma para projeção automatizada de letras de músicas sincronizadas. Projetado para eventos, igrejas e apresentações onde a intervenção manual do operador deve ser mínima.

## Arquitetura e Fluxo

O sistema automatiza o processo de projeção seguindo este pipeline:

1. **Entrada**: O usuário insere a URL de um vídeo do YouTube ou YouTube Music.
2. **Processamento**: O áudio é extraído via `yt-dlp`. As letras sincronizadas são buscadas automaticamente nos provedores suportados (YouTube Captions, LRCLib e LouvorJA).
3. **Fallback Automático (IA)**: Caso nenhuma letra oficial seja encontrada, o sistema aciona o `faster-whisper` localmente para transcrever o áudio gerando os *timestamps* adequados.
4. **Projeção**: A reprodução é feita através da `libmpv`. A letra é renderizada via QML e centralizada no monitor secundário (projetor), enquanto o operador controla o playback pelo monitor primário.

## Funcionalidades

- **Cache Local Persistente**: Áudios e as métricas de sincronismo ficam salvos localmente. Execuções repetidas não consomem banda de internet.
- **Providers de Letras**: Busca nativa nas bases do YouTube (VTT), LRCLib e API LouvorJA. Suporte nativo à importação e exportação de LRC, SRT e VTT.
- **Offset em Tempo Real**: Ajuste fino do timing da legenda (+/- 0.5s) persistido no banco de dados, caso a origem da letra tenha atraso.
- **Clear Screen**: Oculta/exibe o texto no projetor (barra de espaço) útil para passagens instrumentais.
- **Edição in-line**: Correção de erros ortográficos da legenda sendo refletida diretamente na UI e persistida automaticamente no banco.
- **Gestão de Estilo**: Personalização persistente das cores, tamanhos e famílias tipográficas do display.
- **Fila de Reprodução**: Suporte a playlist e recurso de *auto-advance* para transição automatizada entre faixas.

## Stack Tecnológico

- **Core**: Rust (Tokio, sqlx, serde)
- **UI**: Qt 6 + QML (Qt Quick)
- **Playback**: libmpv
- **Processamento de Áudio**: FFmpeg, yt-dlp
- **Transcrição/Fallback**: Python 3 (faster-whisper)
- **Banco de Dados**: SQLite

## Pré-requisitos e Instalação

Para compilar e executar o projeto, instale o compilador Rust e as dependências de sistema do C++ (Qt6 e libmpv) juntamente com as ferramentas de processamento de áudio.

### Fedora Linux (Ambiente de Desenvolvimento)
```bash
sudo dnf install rust cargo qt6-qtdeclarative-devel mpv-libs-devel ffmpeg yt-dlp python3-pip
```

### Ubuntu / Debian
```bash
sudo apt update
sudo apt install cargo qt6-declarative-dev libmpv-dev ffmpeg python3-pip
sudo wget https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -O /usr/local/bin/yt-dlp
sudo chmod a+rx /usr/local/bin/yt-dlp
```

### Windows 11 (Ambiente de Execução Alvo)
A maneira mais controlada de configurar as dependências de C++ no Windows é usando o gerenciador de pacotes Scoop:
```powershell
scoop install git rustup ffmpeg yt-dlp python
scoop bucket add extras
scoop install qtcreator mpv
```
**Atenção no Windows**: Certifique-se de que a `mpv.dll` e as bibliotecas do Qt 6 estejam presentes no seu `PATH` ou que variáveis como `QT_DIR` e `MPV_LIB_DIR` estejam configuradas antes de rodar o `cargo build`.

### Configuração da Inteligência Artificial (Whisper)
Independente do SO, o fallback para a geração de legendas via IA requer o pacote Python local:
```bash
pip install faster-whisper
```

## Execução

O banco de dados SQLite e o download de modelos IA (se necessários) são executados e gerenciados de forma transparente no primeiro boot da aplicação.

Para rodar em ambiente de desenvolvimento habilitando os logs das operações assíncronas (download e transcrição):
```bash
RUST_LOG=info cargo run
```
*(No PowerShell do Windows: `$env:RUST_LOG="info"; cargo run`)*

**Para uso real (Release):**
Recomenda-se rodar em modo otimizado. Isso acelera significativamente o uso do processador pelo `faster-whisper`:
```bash
RUST_LOG=info cargo run --release
```

## Testes

O projeto contém uma suíte cobrindo regras de conversão, timeline e orquestração. Rode localmente com:
```bash
cargo test
```
