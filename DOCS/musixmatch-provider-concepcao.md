# Musixmatch Provider — Concepção Técnica (Não Implementado)

> **Status:** Concepção técnica documentada. **Não implementado** no MVP.
> Motivo da exclusão: viola os Termos de Serviço do Musixmatch e apresenta risco de banimento de IP em ambientes de uso real.

---

## Por que foi considerado

O Musixmatch possui a maior base de letras sincronizadas do mundo. É o provider oficial do Spotify, Apple Music e Amazon Music. Para músicas gospel brasileiras (principal caso de uso deste projeto), a cobertura é significativamente maior que o LRCLib ou LouvorJA para títulos menos populares ou álbuns independentes.

---

## Como funcionaria tecnicamente

O Musixmatch não possui API pública gratuita com acesso a letras sincronizadas. Toda implementação sem chave oficial usa um token extraído do APK do app Android do Musixmatch.

### Fluxo de acesso (API não oficial)

```
1. GET https://apic-desktop.musixmatch.com/ws/1.1/token.get
   Params: app_id=web-desktop-app-v1.0
   → Retorna: user_token (JWT temporário)

2. GET https://apic-desktop.musixmatch.com/ws/1.1/track.search
   Params: q_track={titulo}&q_artist={artista}&usertoken={token}&app_id=...
   → Retorna: lista de tracks com track_id

3. GET https://apic-desktop.musixmatch.com/ws/1.1/track.subtitle.get
   Params: track_id={id}&subtitle_format=lrc&usertoken={token}&app_id=...
   → Retorna: letra sincronizada no formato LRC
```

### Crate Rust disponível

Existe a crate [`musixmatch-inofficial`](https://crates.io/crates/musixmatch-inofficial) que abstrai esse fluxo:

```toml
[dependencies]
musixmatch-inofficial = "0.x"
```

```rust
use musixmatch_inofficial::Musixmatch;

let client = Musixmatch::new();
let token  = client.get_token().await?;
let track  = client.track_search("Oceans", "Hillsong", &token).await?;
let lrc    = client.get_synced_lyrics(track.id, &token).await?;
```

O output `lrc` já é uma `String` no formato LRC padrão, compatível com o `lrc_parser::parse()` já existente no projeto.

---

## Por que foi descartado

| Critério | Situação |
|---|---|
| Termos de Serviço | **Violação direta** — Musixmatch proíbe acesso sem chave oficial |
| Risco operacional | Ban por IP frequente em uso intensivo; sem SLA |
| Validade do token | Tokens expiram e exigem renovação automática |
| Uso comercial/público | Inviável sem licença paga |
| Alternativa superior | LRCLib + LouvorJA + Whisper cobrem 95% do escopo gospel |

---

## Quando reconsiderar

Se o projeto evoluir para uma distribuição pública e a equipe conseguir uma **chave oficial de desenvolvedor Musixmatch** (gratuita para projetos não-comerciais via developer.musixmatch.com), a integração via API oficial seria trivial:

```
GET https://api.musixmatch.com/ws/1.1/track.search?q_track=X&apikey=KEY
GET https://api.musixmatch.com/ws/1.1/track.subtitle.get?track_id=Y&apikey=KEY
```

Nesse caso, o provider seguiria o mesmo padrão de implementação do `LouvorJaProvider` (dois requests: busca → ID → detalhe), com parsing do LRC já suportado pelo `lrc_parser` existente.
