#![allow(dead_code)]

mod application;
mod domain;
mod infrastructure;
mod presentation;
mod shared;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("Letras Sync iniciado com sucesso.");

    match shared::config::load_settings() {
        Ok(settings) => {
            let path = shared::config::config_file_path();
            tracing::info!("Configurações carregadas: {settings:?}");
            match path {
                Ok(path) => tracing::info!("Arquivo de configuração: {}", path.display()),
                Err(err) => tracing::warn!("Não foi possível resolver o caminho do arquivo de configuração: {err:?}"),
            }
        }
        Err(err) => tracing::error!("Falha ao carregar as configurações: {err:?}"),
    }

    infrastructure::database::Database::new().await?;
    tracing::info!("Banco de dados conectado e migrações aplicadas com sucesso.");

    presentation::ui::run_test_window()?;

    Ok(())
}
