use qmetaobject::prelude::*;

qrc!(register_qml_resources,
    "letras_sync/presentation" {
        "src/presentation/test.qml" as "test.qml",
    },
);

/// Verifica se há um servidor gráfico ativo (X11/Wayland).
fn has_display() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}

/// PoC: carrega o QML de teste e exibe a janela.
///
/// Em ambientes sem servidor gráfico (ex.: testes automatizados headless),
/// registra um aviso e retorna sem abrir a janela, evitando quebra.
pub fn run_test_window() -> anyhow::Result<()> {
    if !has_display() {
        tracing::warn!(
            "Nenhum servidor gráfico ativo (WAYLAND_DISPLAY/DISPLAY ausente); \
             pulando a janela de teste QML."
        );
        return Ok(());
    }

    register_qml_resources();

    let mut engine = QmlEngine::new();
    engine.load_file("qrc:/letras_sync/presentation/test.qml".into());
    tracing::info!("Motor QML inicializado; exibindo janela de teste.");
    engine.exec();

    Ok(())
}
