//! Utilidades de WebView2.

/// Marca el WebView de una ventana como visible o no. Una ventana ocultada con `hide()` sigue teniendo su WebView
/// «visible» para WebView2: sigue ejecutando animaciones CSS y temporizadores a pleno ritmo aunque nadie la vea. Con
/// `IsVisible = false` WebView2 deja de pintar y frena los temporizadores. Se ejecuta en el hilo principal.
pub fn set_visible(window: &tauri::WebviewWindow, visible: bool) {
    #[cfg(windows)]
    {
        let _ = window.with_webview(move |webview| unsafe {
            let _ = webview.controller().SetIsVisible(visible);
        });
    }
    #[cfg(not(windows))]
    let _ = (window, visible);
}
