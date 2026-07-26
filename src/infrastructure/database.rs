use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

const DB_FILE_NAME: &str = "letras_sync.db";

/// Caminho absoluto do arquivo do banco no diretório de dados locais do SO.
fn db_file_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from("", "LetrasSync", "LetrasSync")
        .context("não foi possível resolver os diretórios do projeto para o sistema operacional")?;

    Ok(project_dirs.data_local_dir().join(DB_FILE_NAME))
}

/// Encapsula o pool de conexões com o banco SQLite.
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Inicializa a conexão com o banco, criando o arquivo e aplicando as
    /// migrações embutidas caso necessário.
    pub async fn new() -> Result<Self> {
        let path = db_file_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("falha ao criar o diretório {}", parent.display()))?;
        }

        tracing::info!("Conectando ao banco de dados em {}", path.display());

        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options)
            .await
            .with_context(|| format!("falha ao conectar ao banco de dados {}", path.display()))?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .context("falha ao aplicar as migrações do banco de dados")?;

        Ok(Self { pool })
    }

    /// Referência ao pool de conexões subjacente.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
