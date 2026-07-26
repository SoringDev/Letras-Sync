use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::domain::settings::Settings;

const CONFIG_FILE_NAME: &str = "config.toml";

fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from("", "LetrasSync", "LetrasSync")
        .context("não foi possível resolver os diretórios do projeto para o sistema operacional")
}

/// Caminho absoluto do arquivo `config.toml` no diretório de configuração do SO.
pub fn config_file_path() -> Result<PathBuf> {
    Ok(project_dirs()?.config_dir().join(CONFIG_FILE_NAME))
}

fn default_cache_path() -> Result<String> {
    Ok(project_dirs()?
        .data_local_dir()
        .join("cache")
        .to_string_lossy()
        .into_owned())
}

/// Carrega as configurações do arquivo TOML.
///
/// Se o arquivo não existir ou for inválido, gera as configurações padrão,
/// cria os diretórios necessários e grava o arquivo no disco.
pub fn load_settings() -> Result<Settings> {
    let path = config_file_path()?;

    match fs::read_to_string(&path) {
        Ok(contents) => match toml::from_str::<Settings>(&contents) {
            Ok(settings) => Ok(settings),
            Err(err) => {
                tracing::warn!(
                    "arquivo de configuração inválido ({}): {err}. Recriando com valores padrão.",
                    path.display()
                );
                let settings = default_settings()?;
                save_settings(&settings)?;
                Ok(settings)
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!(
                "arquivo de configuração não encontrado ({}). Criando com valores padrão.",
                path.display()
            );
            let settings = default_settings()?;
            save_settings(&settings)?;
            Ok(settings)
        }
        Err(err) => Err(err).with_context(|| {
            format!("falha ao ler o arquivo de configuração {}", path.display())
        }),
    }
}

/// Grava as configurações no arquivo TOML, criando os diretórios se necessário.
pub fn save_settings(settings: &Settings) -> Result<()> {
    let path = config_file_path()?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("falha ao criar o diretório {}", parent.display()))?;
    }

    let contents = toml::to_string_pretty(settings)
        .context("falha ao serializar as configurações para TOML")?;

    fs::write(&path, contents)
        .with_context(|| format!("falha ao gravar o arquivo de configuração {}", path.display()))?;

    Ok(())
}

fn default_settings() -> Result<Settings> {
    Ok(Settings {
        cache_path: default_cache_path()?,
        ..Settings::default()
    })
}
