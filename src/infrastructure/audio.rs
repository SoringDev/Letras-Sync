use anyhow::Result;
use libmpv::Mpv;

/// Fachada fina sobre o libmpv que expõe controles básicos de reprodução de
/// áudio e a consulta da posição atual.
pub struct AudioEngine {
    mpv: Mpv,
}

impl AudioEngine {
    /// Inicializa o libmpv em modo somente áudio.
    pub fn new() -> Result<Self> {
        let mpv = Mpv::new().map_err(|e| anyhow::anyhow!("{e:?}"))?;

        mpv.set_property("video", false)
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        mpv.set_property("keep-open", true)
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;

        Ok(Self { mpv })
    }

    /// Carrega a URL informada substituindo a reprodução atual.
    pub fn load(&self, url: &str) -> Result<()> {
        self.mpv
            .command("loadfile", &[url, "replace"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))
    }

    /// Retoma a reprodução.
    pub fn play(&self) -> Result<()> {
        self.mpv
            .set_property("pause", false)
            .map_err(|e| anyhow::anyhow!("{e:?}"))
    }

    /// Pausa a reprodução.
    pub fn pause(&self) -> Result<()> {
        self.mpv
            .set_property("pause", true)
            .map_err(|e| anyhow::anyhow!("{e:?}"))
    }

    /// Interrompe a reprodução.
    pub fn stop(&self) -> Result<()> {
        self.mpv
            .command("stop", &[])
            .map_err(|e| anyhow::anyhow!("{e:?}"))
    }

    /// Move a reprodução para a posição absoluta em segundos.
    pub fn seek(&self, seconds: f64) -> Result<()> {
        self.mpv
            .command("seek", &[&seconds.to_string(), "absolute"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))
    }

    /// Move a reprodução em `delta_seconds` relativos à posição atual.
    pub fn seek_relative(&self, delta_seconds: f64) -> Result<()> {
        self.mpv
            .command("seek", &[&delta_seconds.to_string(), "relative"])
            .map_err(|e| anyhow::anyhow!("{e:?}"))
    }

    /// Posição atual em segundos. Retorna 0.0 quando ainda não disponível.
    pub fn position(&self) -> f64 {
        self.mpv.get_property("time-pos").unwrap_or(0.0)
    }

    /// Duração total em segundos, ou None quando não disponível.
    pub fn duration(&self) -> Option<f64> {
        self.mpv.get_property("duration").ok()
    }

    /// Indica se o mpv está ocioso (sem mídia em reprodução).
    ///
    /// Retorna `true` quando a propriedade `idle-active` estiver ativa ou
    /// quando não for possível consultá-la.
    pub fn is_idle(&self) -> bool {
        self.mpv.get_property("idle-active").unwrap_or(true)
    }
}
