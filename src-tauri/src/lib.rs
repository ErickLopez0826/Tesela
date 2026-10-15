//! Raíz de composición: cablea las capas (dominio ← aplicación ← infraestructura/interfaz).

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![interface::commands::list_windows])
        .run(tauri::generate_context!())
        .expect("error al ejecutar la aplicación");
}
