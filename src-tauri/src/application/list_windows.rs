use crate::application::ports::WindowEnumerator;
use crate::domain::window::WindowInfo;

/// Caso de uso: listar las ventanas visibles del sistema.
pub fn list_windows(enumerator: &dyn WindowEnumerator) -> Result<Vec<WindowInfo>, String> {
    Ok(enumerator
        .enumerate()?
        .into_iter()
        .filter(|c| c.is_listable())
        .map(WindowInfo::from)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::window::WindowCandidate;

    struct Fake(Result<Vec<WindowCandidate>, String>);

    impl WindowEnumerator for Fake {
        fn enumerate(&self) -> Result<Vec<WindowCandidate>, String> {
            self.0.clone()
        }
    }

    fn cand(handle: isize, title: &str, visible: bool) -> WindowCandidate {
        WindowCandidate {
            handle,
            exe: "app.exe".into(),
            title: title.into(),
            visible,
            cloaked: false,
            tool_window: false,
            own_process: false,
        }
    }

    #[test]
    fn filtra_y_conserva_el_orden() {
        let fake = Fake(Ok(vec![cand(1, "A", true), cand(2, "B", false), cand(3, "C", true)]));
        let handles: Vec<_> = list_windows(&fake).unwrap().iter().map(|w| w.handle).collect();
        assert_eq!(handles, vec![1, 3]);
    }

    #[test]
    fn lista_vacia_no_falla() {
        assert!(list_windows(&Fake(Ok(vec![]))).unwrap().is_empty());
    }

    #[test]
    fn propaga_el_error_del_enumerador() {
        assert_eq!(list_windows(&Fake(Err("x".into()))).unwrap_err(), "x");
    }
}
