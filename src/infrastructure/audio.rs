use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};
use std::ptr::NonNull;

use anyhow::{bail, Context, Result};
use libmpv_sys as mpv;

/// Fachada fina sobre o libmpv que expõe controles básicos de reprodução de
/// áudio e a consulta da posição atual.
pub struct AudioEngine {
    ctx: NonNull<mpv::mpv_handle>,
}

// O handle do libmpv é acessado apenas por métodos síncronos desta fachada.
// O próprio libmpv gerencia a coordenação interna necessária para esse uso.
unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}

impl AudioEngine {
    /// Inicializa o libmpv em modo somente áudio.
    pub fn new() -> Result<Self> {
        let ctx = unsafe { mpv::mpv_create() };
        let ctx = NonNull::new(ctx).context("falha ao criar contexto do libmpv")?;

        let engine = Self { ctx };
        engine.set_option_string("video", "no")?;
        engine.set_option_string("keep-open", "yes")?;
        engine.initialize()?;

        Ok(engine)
    }

    /// Carrega a URL informada substituindo a reprodução atual.
    pub fn load(&self, url: &str) -> Result<()> {
        self.command(&["loadfile", url, "replace"])
    }

    /// Retoma a reprodução.
    pub fn play(&self) -> Result<()> {
        self.set_property_string("pause", "no")
    }

    /// Pausa a reprodução.
    pub fn pause(&self) -> Result<()> {
        self.set_property_string("pause", "yes")
    }

    /// Interrompe a reprodução.
    pub fn stop(&self) -> Result<()> {
        self.command(&["stop"])
    }

    /// Ajusta o volume em porcentagem.
    pub fn set_volume(&self, percent: i64) -> Result<()> {
        let mut value = percent;
        let name = CString::new("volume").context("nome de propriedade inválido para libmpv")?;

        check_mpv(
            unsafe {
                mpv::mpv_set_property(
                    self.ctx(),
                    name.as_ptr(),
                    mpv::mpv_format_MPV_FORMAT_INT64,
                    &mut value as *mut i64 as *mut c_void,
                )
            },
            "falha ao definir volume no libmpv",
        )
    }

    /// Lê o volume atual em porcentagem. Retorna 100 quando indisponível.
    pub fn volume(&self) -> i64 {
        self.get_property_int64("volume").unwrap_or(100)
    }

    /// Move a reprodução para a posição absoluta em segundos.
    pub fn seek(&self, seconds: f64) -> Result<()> {
        self.command(&["seek", &seconds.to_string(), "absolute"])
    }

    /// Move a reprodução em `delta_seconds` relativos à posição atual.
    pub fn seek_relative(&self, delta_seconds: f64) -> Result<()> {
        self.command(&["seek", &delta_seconds.to_string(), "relative"])
    }

    /// Posição atual em segundos. Retorna 0.0 quando ainda não disponível.
    pub fn position(&self) -> f64 {
        self.get_property_string("time-pos")
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0)
    }

    /// Duração total em segundos, ou None quando não disponível.
    pub fn duration(&self) -> Option<f64> {
        self.get_property_string("duration")
            .and_then(|value| value.parse::<f64>().ok())
    }

    /// Indica se o mpv está ocioso (sem mídia em reprodução).
    ///
    /// Retorna `true` quando a propriedade `idle-active` estiver ativa ou
    /// quando não for possível consultá-la.
    pub fn is_idle(&self) -> bool {
        self.get_property_string("idle-active")
            .map(|value| matches!(value.as_str(), "yes" | "true" | "1"))
            .unwrap_or(true)
    }

    fn initialize(&self) -> Result<()> {
        check_mpv(unsafe { mpv::mpv_initialize(self.ctx()) }, "falha ao inicializar o libmpv")
    }

    fn set_option_string(&self, name: &str, value: &str) -> Result<()> {
        let name = CString::new(name).context("nome de opção inválido para libmpv")?;
        let value = CString::new(value).context("valor de opção inválido para libmpv")?;

        check_mpv(
            unsafe { mpv::mpv_set_option_string(self.ctx(), name.as_ptr(), value.as_ptr()) },
            "falha ao configurar opção do libmpv",
        )
    }

    fn set_property_string(&self, name: &str, value: &str) -> Result<()> {
        let name = CString::new(name).context("nome de propriedade inválido para libmpv")?;
        let value = CString::new(value).context("valor de propriedade inválido para libmpv")?;

        check_mpv(
            unsafe { mpv::mpv_set_property_string(self.ctx(), name.as_ptr(), value.as_ptr()) },
            "falha ao definir propriedade do libmpv",
        )
    }

    fn get_property_string(&self, name: &str) -> Option<String> {
        let name = CString::new(name).ok()?;
        let raw = unsafe { mpv::mpv_get_property_string(self.ctx(), name.as_ptr()) };
        let raw = NonNull::new(raw)?;

        let value = unsafe { CStr::from_ptr(raw.as_ptr()) }
            .to_string_lossy()
            .into_owned();

        unsafe {
            mpv::mpv_free(raw.as_ptr() as *mut c_void);
        }

        Some(value)
    }

    fn get_property_int64(&self, name: &str) -> Option<i64> {
        let name = CString::new(name).ok()?;
        let mut value: i64 = 0;

        let code = unsafe {
            mpv::mpv_get_property(
                self.ctx(),
                name.as_ptr(),
                mpv::mpv_format_MPV_FORMAT_INT64,
                &mut value as *mut i64 as *mut c_void,
            )
        };

        if code < 0 {
            return None;
        }

        Some(value)
    }

    fn command(&self, args: &[&str]) -> Result<()> {
        let c_args: Vec<CString> = args
            .iter()
            .map(|arg| CString::new(*arg).context("argumento inválido para comando do libmpv"))
            .collect::<Result<_>>()?;

        let mut ptrs: Vec<*const c_char> = c_args.iter().map(|arg| arg.as_ptr()).collect();
        ptrs.push(std::ptr::null());

        check_mpv(
            unsafe { mpv::mpv_command(self.ctx(), ptrs.as_mut_ptr()) },
            "falha ao executar comando do libmpv",
        )
    }

    fn ctx(&self) -> *mut mpv::mpv_handle {
        self.ctx.as_ptr()
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        unsafe {
            mpv::mpv_terminate_destroy(self.ctx.as_ptr());
        }
    }
}

fn check_mpv(code: i32, context: &str) -> Result<()> {
    if code < 0 {
        let message = unsafe {
            let ptr = mpv::mpv_error_string(code);
            if ptr.is_null() {
                format!("erro desconhecido do libmpv ({code})")
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };

        bail!("{context}: {message}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_can_be_set_and_read_when_mpv_is_available() {
        let Ok(engine) = AudioEngine::new() else {
            return;
        };

        let original = engine.volume();
        engine.set_volume(73).expect("set volume");

        assert_eq!(engine.volume(), 73);

        let _ = engine.set_volume(original);
    }
}
