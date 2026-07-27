# Letras Sync

**Letras Sync** é um Sistema de Projeção Inteligente de Letras Sincronizadas. O projeto é uma aplicação desktop desenvolvida para receber um link de uma música do YouTube, extrair automaticamente o áudio e a letra, e projetar o texto em um segundo monitor (projetor) perfeitamente sincronizado com a reprodução da música.

## 🎯 O que ele resolve?

O objetivo principal é minimizar a intervenção de um operador na projeção de letras (como em igrejas ou eventos). O sistema automatiza o fluxo: busca os metadados, baixa a música, obtém a legenda oficial ou utiliza Inteligência Artificial (Whisper) para gerar uma letra sincronizada caso não exista.

## 🛠 Como Funciona (Fluxo Principal)

1. O usuário cola a URL do vídeo do YouTube.
2. O sistema extrai o áudio via yt-dlp e busca por letras disponíveis através de Providers (YouTube Captions e LRCLib).
3. Se não encontrar letras, aciona o **faster-whisper** como fallback para ouvir o áudio e gerar a letra sincronizada.
4. O áudio é reproduzido pelo libmpv enquanto a letra é projetada em tela cheia no segundo monitor (texto amarelo em fundo preto via QML).

## ✨ Funcionalidades

O **Letras Sync** conta com uma suíte de funcionalidades projetada para automatizar e simplificar a projeção de letras em tempo real:

### 1. Obtenção e Cache de Mídia
* **Integração com YouTube**: Basta colar a URL de um vídeo do YouTube para que o sistema processe e faça o download automático do áudio em segundo plano via `yt-dlp`.
* **Metadados Automáticos**: Extração automática do título, artista e thumbnail da mídia.
* **Cache Inteligente**: O áudio baixado é armazenado em formato `.mp3` localmente. Execuções subsequentes da mesma música utilizam o cache físico, eliminando a necessidade de internet.

### 2. Motor de Letras Sincronizadas (Lyrics Engine)
* **Múltiplos Provedores (Providers)**: Busca automática de letras sincronizadas através do YouTube Captions e da API do LRCLib.
* **Fallback com Inteligência Artificial (Whisper)**: Caso a música não possua letras sincronizadas na internet, o sistema aciona o `faster-whisper` localmente para analisar o áudio e transcrever a letra com marcações de tempo precisas.
* **Suporte a Múltiplos Formatos**: Compatibilidade com os formatos de legenda mais comuns: LRC, SRT e VTT.

### 3. Painel do Operador e Controle de Playlist
* **Player de Áudio Robusto**: Controle completo de reprodução (Play, Pause, Stop) acionado por botões e atalhos globais, integrado via `libmpv`.
* **Navegação Temporal**: Barra de progresso interativa e botões de avanço/retrocesso rápido (+/- 10 segundos).
* **Fila de Reprodução (Playlist)**: Adicione músicas à playlist ativa com suporte a avanço automático (*auto-advance*) de faixas.
* **Histórico Persistente**: Histórico de reprodução salvo localmente com indicador visual do status de sincronismo ("Letra Salva" vs "Pendente Whisper").
* **Busca e Filtro**: Campo de busca em tempo real para filtrar o histórico por título ou artista.

### 4. Edição e Ajuste de Sincronismo
* **Edição Rápida de Letras**: Permite que o operador corrija erros gramaticais ou ajuste textos da linha ativa em tempo real diretamente na interface, persistindo a alteração no banco imediatamente.
* **Ajuste Fino de Offset**: Ferramenta para atrasar ou adiantar a exibição da letra em tempo real (+/- 0.5s) para garantir sincronia cirúrgica com o áudio.
* **Limpeza de Cache**: Opção de forçar a limpeza do cache de letras de uma música específica para reiniciar o processo de busca ou transcrição.

### 5. Projeção e Estilização
* **Detecção de Segundo Monitor**: Direciona automaticamente a projeção em tela cheia para o monitor secundário (projetor ou TV) configurado.
* **Visualização de Alto Contraste**: Fundo preto com texto amarelo puro para máxima legibilidade à distância.
* **Modo Ocultar Texto (Clear Screen)**: Permite limpar o projetor instantaneamente (por exemplo, em momentos de silêncio na música) com um único clique ou pressionando a barra de espaço, mantendo o áudio tocando.
* **Customização de Estilo**: Painel completo para ajustar o tamanho da fonte, família de fontes, cor do texto e cor de fundo da tela de projeção.
* **Persistência de Estilo**: Configurações de estilo e tamanho de fonte são gravadas no disco e recarregadas automaticamente na próxima execução.

### 6. Importação e Exportação
* **Exportação Local**: Permite salvar a letra da música ativa em arquivos locais nos formatos `.lrc` ou `.srt`.
* **Importação Local**: Permite carregar arquivos de sincronismo `.lrc`, `.srt` ou `.vtt` salvos no seu computador diretamente no banco de dados do aplicativo.

### 7. Operação Ágil via Atalhos de Teclado
* **Espaço**: Oculta ou exibe o texto no projetor (Clear Screen).
* **Seta Esquerda / Seta Direita**: Retrocede ou avança 10 segundos no áudio.
* **Seta Cima / Seta Baixo**: Ajusta o volume (+/- 5%).
* **PageUp / PageDown**: Alterna para a música anterior ou posterior na playlist.
* **Ctrl++ / Ctrl+-**: Aumenta ou diminui o tamanho da fonte da projeção.
* **Esc**: Para a execução e limpa o player (Stop).

---

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

---

## 🚀 Como Iniciar e Rodar o App

Siga o passo a passo abaixo para clonar, configurar as dependências, executar e testar o aplicativo no seu sistema operacional.

### Passo 1: Clonar o Repositório

Primeiro, clone o repositório para o seu ambiente local e acesse a pasta do projeto:

```bash
git clone https://github.com/SoringDev/Letras-Sync.git
cd Letras-Sync
```

### Passo 2: Instalar os Pré-requisitos do Sistema

Para compilar e executar o projeto, você precisará do compilador Rust, Qt 6 (com suporte a Declarative/QML), libmpv (para o player de áudio), FFmpeg, yt-dlp e Python 3.

#### 🐧 No Fedora Linux (Ambiente de Desenvolvimento)
```bash
sudo dnf install rust cargo qt6-qtdeclarative-devel mpv-libs-devel ffmpeg yt-dlp python3-pip
```

#### 🐧 No Ubuntu / Debian
```bash
sudo apt update
sudo apt install cargo qt6-declarative-dev libmpv-dev ffmpeg python3-pip
sudo wget https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -O /usr/local/bin/yt-dlp
sudo chmod a+rx /usr/local/bin/yt-dlp
```

#### 🪟 No Windows 11 (Ambiente de Execução Final)
Recomenda-se utilizar o gerenciador de pacotes **Scoop** para facilitar a instalação:
```powershell
# Instalar dependências básicas
scoop install git rustup ffmpeg yt-dlp python

# Instalar Qt 6 e mpv
scoop bucket add extras
scoop install qtcreator mpv
```
> [!NOTE]
> No Windows, certifique-se de que a biblioteca do `mpv` (`mpv.dll` ou `mpv.lib`) e as DLLs do Qt 6 estejam no seu `PATH` ou que as variáveis de ambiente necessárias (como `QT_DIR` e `MPV_LIB_DIR`) estejam devidamente configuradas para que o Cargo consiga compilar e linkar as dependências corretas.

---

### Passo 3: Configurar o Fallback de Inteligência Artificial (Whisper)

Para que o fallback de geração de legendas via Whisper funcione corretamente, certifique-se de que o Python possui a biblioteca `faster-whisper` instalada:
```bash
pip install faster-whisper
```

---

### Passo 4: Executar a Aplicação

Com todas as dependências e o Rust configurados, você pode iniciar a aplicação usando o Cargo:

#### 1. Modo Padrão (Sem Logs Visíveis)
```bash
cargo run
```

#### 2. Modo com Logs Ativos (Recomendado para Desenvolvimento)
O aplicativo utiliza o crate `tracing` para emitir informações no console. Defina a variável `RUST_LOG` antes de executar para ver o progresso do download do YouTube e transcrição da IA:
```bash
# No Linux (Bash)
RUST_LOG=info cargo run

# No Windows (PowerShell)
$env:RUST_LOG="info"; cargo run
```

#### 3. Modo Release (Uso em Produção/Real)
Para um desempenho significativamente melhor e menor uso de CPU durante o processamento do áudio e transcrição com Whisper, execute em modo otimizado:
```bash
# No Linux
RUST_LOG=info cargo run --release

# No Windows
$env:RUST_LOG="info"; cargo run --release
```

---

### 💡 Primeiras Execuções e Inicialização Automática

* **Banco de Dados**: O banco de dados SQLite local (`letras_sync.db`) e todas as migrações de tabelas necessárias serão inicializados automaticamente no primeiro boot do aplicativo no diretório padrão de dados do sistema.
* **Modelo do Whisper**: Na primeira vez que for necessário transcrever uma música que não possui letra sincronizada na internet, o aplicativo fará o download automático do modelo do Whisper (`medium`) a partir do Hugging Face. Esse download inicial pode demorar alguns minutos dependendo de sua conexão.

---

### Passo 5: Executar os Testes Automatizados

O projeto conta com testes automatizados de unidade e integração. Para executá-los em modo headless (sem necessidade de interface gráfica ativa):
```bash
cargo test
```


