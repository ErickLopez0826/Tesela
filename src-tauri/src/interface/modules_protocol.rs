//! Protocolo `wmmodule://`: sirve los archivos de los módulos con una política de seguridad (CSP)
//! calculada por el núcleo según lo que el usuario concedió. En Windows el webview lo llama como
//! `http://wmmodule.localhost/<id>/<ruta>`.

use tauri::http::Response;

use crate::infrastructure::module_store::{ModuleStore, Served};

/// Separa `/<id>/<ruta...>`; la ruta puede quedar vacía (entonces se sirve la página de entrada).
pub fn split_path(path: &str) -> Option<(&str, &str)> {
    let trimmed = path.trim_start_matches('/');
    let (id, rest) = trimmed.split_once('/').unwrap_or((trimmed, ""));
    (!id.is_empty()).then_some((id, rest))
}

pub fn build_response(served: Served) -> Response<Vec<u8>> {
    let mut b = Response::builder()
        .status(served.status)
        .header("Content-Type", served.content_type)
        // El módulo vive en un iframe aislado (origen opaco): necesita CORS abierto solo para leer sus propios archivos.
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff");
    if let Some(csp) = served.csp {
        b = b.header("Content-Security-Policy", csp);
    }
    b.body(served.body).unwrap_or_else(|_| Response::new(Vec::new()))
}

pub fn respond(store: &ModuleStore, path: &str) -> Response<Vec<u8>> {
    let served = match split_path(path) {
        Some((id, rest)) => store.serve(id, rest),
        None => store.serve("", ""),
    };
    build_response(served)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separa_id_y_ruta() {
        assert_eq!(split_path("/hello/index.html"), Some(("hello", "index.html")));
        assert_eq!(split_path("/hello/assets/app.js"), Some(("hello", "assets/app.js")));
        assert_eq!(split_path("/hello"), Some(("hello", "")));
        assert_eq!(split_path("/hello/"), Some(("hello", "")));
        assert_eq!(split_path("/"), None);
        assert_eq!(split_path(""), None);
    }

    #[test]
    fn la_respuesta_lleva_csp_y_tipo() {
        let r = build_response(Served {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: b"x".to_vec(),
            csp: Some("default-src 'none'".into()),
        });
        assert_eq!(r.status(), 200);
        assert_eq!(r.headers()["Content-Security-Policy"], "default-src 'none'");
        assert_eq!(r.headers()["Content-Type"], "text/html; charset=utf-8");
        assert_eq!(r.headers()["X-Content-Type-Options"], "nosniff");
        assert_eq!(r.body(), b"x");
    }
}
