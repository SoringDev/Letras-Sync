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
        Err(err) => Err(err)
            .with_context(|| format!("falha ao ler o arquivo de configuração {}", path.display())),
    }
}

/// Grava as configurações no arquivo TOML, criando os diretórios se necessário.
pub fn save_settings(settings: &Settings) -> Result<()> {
    write_settings(&config_file_path()?, settings)
}

/// Serializa e grava as configurações no caminho informado, criando os
/// diretórios pai se necessário.
fn write_settings(path: &std::path::Path, settings: &Settings) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("falha ao criar o diretório {}", parent.display()))?;
    }

    let contents = toml::to_string_pretty(settings)
        .context("falha ao serializar as configurações para TOML")?;

    fs::write(path, contents).with_context(|| {
        format!(
            "falha ao gravar o arquivo de configuração {}",
            path.display()
        )
    })?;

    Ok(())
}

fn default_settings() -> Result<Settings> {
    Ok(Settings {
        cache_path: default_cache_path()?,
        ..Settings::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_settings_are_reloaded_correctly() {
        let dir = std::env::temp_dir().join(format!(
            "letras_sync_settings_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("diretório temporário");
        let path = dir.join(CONFIG_FILE_NAME);

        let settings = Settings {
            font_size: 72,
            font_family: "Serif".to_string(),
            font_color: "#123456".to_string(),
            background_color: "#654321".to_string(),
            projector_monitor: Some(2),
            cache_path: "/tmp/custom-cache".to_string(),
            volume: 80,
        };

        write_settings(&path, &settings).expect("gravar configurações");

        let contents = fs::read_to_string(&path).expect("ler configurações");
        let reloaded: Settings = toml::from_str(&contents).expect("desserializar");

        assert_eq!(reloaded, settings);

        let _ = fs::remove_dir_all(&dir);
    }
}
