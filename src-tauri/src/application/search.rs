//! Casos de uso del buscador (§18.9): «buscar» une las fuentes y ordena; «abrir» enfoca, lanza o abre una dirección.
//!
//! La interfaz nunca manda rutas ni direcciones: solo el identificador de un resultado que este servicio acaba de ofrecer.
//! El servicio recuerda a qué corresponde cada identificador y, si no lo reconoce, no hace nada (§18.10).

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use crate::domain::search::{
    fold, host_of, is_http_url, norm, opens_new_window, origin_of, parse_web_search, rank, Candidate, Redirect, RedirectKind, Ranked, SearchItem, SearchKind, UsageEntry,
    WEB_SEARCH_SCORE,
};
use crate::domain::packaged;
use crate::domain::windows_settings;
use crate::domain::startup::{exe_of_path, AppSuggestion};
use crate::domain::utility::{is_launchable, OpenApp};

/// ¿Se puede abrir esto? Un `.exe` o acceso directo con ruta absoluta, o la dirección de una app empaquetada (Teams…).
fn can_launch(path: &str) -> Result<(), String> {
    if packaged::is_launch_path(path) {
        packaged::validate_launch_path(path)
    } else {
        is_launchable(path)
    }
}

/// Puerto: lo que el buscador necesita del sistema.
pub trait SearchHost: Send {
    /// Aplicaciones instaladas y abiertas, ya unidas (cada una marcada si está abierta).
    fn apps(&mut self) -> Vec<AppSuggestion>;
    /// Ventanas abiertas ahora, para saber cuál enfocar.
    fn open_apps(&mut self) -> Vec<OpenApp>;
    /// Cambia al workspace de la ventana si hace falta y le da el foco.
    fn focus(&mut self, handle: isize) -> Result<(), String>;
    /// Abre un programa o acceso directo; con `as_admin`, pidiendo elevación a Windows (que muestra su aviso).
    /// Si el usuario cancela ese aviso no es un error.
    fn launch(&mut self, path: &str, as_admin: bool) -> Result<(), String>;
    /// Abre una dirección web en el navegador predeterminado (ya validada como http/https).
    fn open_url(&mut self, url: &str) -> Result<(), String>;
}

/// A qué corresponde un identificador de resultado.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Launch { path: String },
    Focus { handle: isize, path: String },
    Url { url: String },
    /// Una página de la Configuración de Windows (`ms-settings:…`).
    Setting { uri: String },
}

#[derive(Debug, Clone)]
struct Entry {
    target: Target,
    /// Clave del historial de uso.
    key: String,
}

/// Letras mínimas para que salgan páginas de la Configuración de Windows: con una sola, sus decenas de nombres taparían a las aplicaciones.
const MIN_SETTINGS_QUERY: usize = 2;

/// Cuántos identificadores se recuerdan: las consultas llegan mientras se escribe y la interfaz descarta las viejas, pero
/// la respuesta de una consulta anterior puede terminar después; así los identificadores de ambas siguen siendo válidos.
const REMEMBERED_IDS: usize = 256;

#[derive(Default)]
struct Issued {
    next: u64,
    entries: HashMap<String, Entry>,
    order: VecDeque<String>,
}

impl Issued {
    fn issue(&mut self, entry: Entry) -> String {
        self.next += 1;
        let id = format!("r{}", self.next);
        self.entries.insert(id.clone(), entry);
        self.order.push_back(id.clone());
        while self.order.len() > REMEMBERED_IDS {
            if let Some(old) = self.order.pop_front() {
                self.entries.remove(&old);
            }
        }
        id
    }
}

#[derive(Default)]
pub struct SearchService {
    issued: Mutex<Issued>,
}

impl SearchService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Busca y ordena. Con el cuadro vacío devuelve las más usadas.
    pub fn query(&self, host: &mut dyn SearchHost, text: &str, redirects: &[Redirect], usage: &HashMap<String, UsageEntry>, now: u64) -> Vec<SearchItem> {
        let open = host.open_apps();
        let mut targets: HashMap<String, Entry> = HashMap::new();
        let mut candidates: Vec<Candidate> = Vec::new();

        for app in host.apps() {
            let key = format!("app:{}:{}", app.exe, fold(&app.name));
            // Un navegador nunca se enfoca: puede estar abierto solo por una app web (WhatsApp Web…) y al elegirlo se espera su
            // ventana de siempre. Se lanza otra ventana.
            let window = (app.running && !opens_new_window(&app.exe)).then(|| open.iter().find(|o| exe_of_path(&o.exe) == app.exe)).flatten();
            // El `.exe` de una app empaquetada vive en `WindowsApps` y Windows no deja ejecutarlo directamente: para reabrirla o sacar
            // su logo se usa siempre su dirección de la Tienda.
            let keep_own_path = packaged::is_launch_path(&app.path);
            let (kind, subtitle, target) = match window {
                Some(w) => (
                    SearchKind::Running,
                    "En ejecución",
                    Target::Focus { handle: w.handle, path: if w.path.is_empty() || keep_own_path { app.path.clone() } else { w.path.clone() } },
                ),
                None => (SearchKind::App, "Aplicación", Target::Launch { path: app.path.clone() }),
            };
            targets.insert(key.clone(), Entry { target, key: key.clone() });
            candidates.push(Candidate { key, kind, title: app.name.clone(), subtitle: subtitle.into(), match_text: app.name, site: None });
        }

        // La Configuración de Windows no tiene acceso directo (es una app UWP): sus páginas salen de una lista fija.
        for page in windows_settings::PAGES {
            // Con una sola letra se dejan fuera: ahí mandan las aplicaciones.
            if !text.trim().is_empty() && text.trim().chars().count() < MIN_SETTINGS_QUERY {
                break;
            }
            let Some(match_text) = windows_settings::matching_name(page, text) else { continue };
            let key = format!("setting:{}", page.uri);
            targets.insert(key.clone(), Entry { target: Target::Setting { uri: page.uri.into() }, key: key.clone() });
            candidates.push(Candidate { key, kind: SearchKind::App, title: page.name.into(), subtitle: "Configuración de Windows".into(), match_text: match_text.into(), site: None });
        }

        for r in redirects.iter().filter(|r| r.kind == RedirectKind::Redirect) {
            let key = format!("web:{}", norm(&r.word));
            targets.insert(key.clone(), Entry { target: Target::Url { url: r.url.clone() }, key: key.clone() });
            candidates.push(Candidate { key, kind: SearchKind::Redirect, title: format!("Abrir {}", host_of(&r.url)), subtitle: r.url.clone(), match_text: r.word.clone(), site: origin_of(&r.url) });
        }

        let mut ranked = rank(text, candidates, usage, now);

        // «palabra texto» de una búsqueda web: va por encima de todo lo demás salvo una redirección exacta.
        if let Some(search) = parse_web_search(text, redirects) {
            let key = format!("web:{}", norm(&search.word));
            targets.insert(key.clone(), Entry { target: Target::Url { url: search.url.clone() }, key: key.clone() });
            let candidate = Candidate {
                key,
                kind: SearchKind::WebSearch,
                title: format!("Buscar «{}»", search.text),
                subtitle: format!("Búsqueda web en {}", host_of(&search.url)),
                site: origin_of(&search.url),
                match_text: search.text,
            };
            let at = ranked.iter().position(|r| r.score < WEB_SEARCH_SCORE).unwrap_or(ranked.len());
            ranked.insert(at, Ranked { candidate, score: WEB_SEARCH_SCORE });
        }

        let mut issued = self.issued.lock().unwrap();
        ranked
            .into_iter()
            .filter_map(|r| {
                let entry = targets.remove(&r.candidate.key)?;
                let id = issued.issue(entry);
                Some(SearchItem { id, kind: r.candidate.kind, title: r.candidate.title, subtitle: r.candidate.subtitle, site: r.candidate.site })
            })
            .collect()
    }

    /// Ruta del programa de un resultado, para pedirle su icono. `None` si el identificador no es válido o no es un programa.
    pub fn icon_path(&self, id: &str) -> Option<String> {
        match &self.issued.lock().unwrap().entries.get(id)?.target {
            Target::Launch { path } | Target::Focus { path, .. } if !path.is_empty() => Some(path.clone()),
            // Todas las páginas de la Configuración llevan el engranaje de la app.
            Target::Setting { .. } => Some(windows_settings::ICON_ADDRESS.into()),
            _ => None,
        }
    }

    /// Abre un resultado. Devuelve la clave del historial de uso si se hizo algo; `Ok(None)` si el identificador no se reconoce.
    pub fn open(&self, host: &mut dyn SearchHost, id: &str, as_admin: bool) -> Result<Option<String>, String> {
        let Some(entry) = self.issued.lock().unwrap().entries.get(id).cloned() else {
            return Ok(None);
        };
        match entry.target {
            Target::Url { url } => {
                if !is_http_url(&url) {
                    return Err("la dirección no es válida".into());
                }
                host.open_url(&url)?;
            }
            Target::Setting { uri } => {
                windows_settings::validate_uri(&uri)?;
                // La Configuración es una app empaquetada: no se puede elevar, se abre siempre normal.
                host.launch(&uri, false)?;
            }
            Target::Launch { path } => {
                can_launch(&path)?;
                host.launch(&path, as_admin)?;
            }
            Target::Focus { handle, path } => {
                // Una app empaquetada no se puede elevar desde fuera: con ctrl+Enter solo se enfoca.
                if as_admin && !packaged::is_launch_path(&path) && is_launchable(&path).is_ok() {
                    host.launch(&path, true)?;
                } else if host.focus(handle).is_err() {
                    // La ventana ya no existe: se abre otra.
                    can_launch(&path)?;
                    host.launch(&path, as_admin)?;
                }
            }
        }
        Ok(Some(entry.key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::search::record_usage;

    #[derive(Default)]
    struct FakeHost {
        apps: Vec<AppSuggestion>,
        open: Vec<OpenApp>,
        focused: Vec<isize>,
        launched: Vec<(String, bool)>,
        urls: Vec<String>,
        focus_fails: bool,
    }

    impl SearchHost for FakeHost {
        fn apps(&mut self) -> Vec<AppSuggestion> {
            self.apps.clone()
        }
        fn open_apps(&mut self) -> Vec<OpenApp> {
            self.open.clone()
        }
        fn focus(&mut self, handle: isize) -> Result<(), String> {
            if self.focus_fails {
                return Err("ya no existe".into());
            }
            self.focused.push(handle);
            Ok(())
        }
        fn launch(&mut self, path: &str, as_admin: bool) -> Result<(), String> {
            self.launched.push((path.into(), as_admin));
            Ok(())
        }
        fn open_url(&mut self, url: &str) -> Result<(), String> {
            self.urls.push(url.into());
            Ok(())
        }
    }

    fn app(name: &str, exe: &str, running: bool) -> AppSuggestion {
        AppSuggestion { name: name.into(), exe: exe.into(), path: format!(r"C:\Menu\{name}.lnk"), running }
    }

    fn window(handle: isize, exe: &str) -> OpenApp {
        OpenApp { handle, exe: format!("{exe}.exe"), path: format!(r"C:\Programas\{exe}.exe"), title: exe.into() }
    }

    fn redirect(word: &str, url: &str, kind: RedirectKind) -> Redirect {
        Redirect { word: word.into(), url: url.into(), kind }
    }

    fn host() -> FakeHost {
        FakeHost {
            apps: vec![app("Spotify", "spotify", true), app("Steam", "steam", false), app("Notepad", "notepad", false)],
            open: vec![window(7, "spotify")],
            ..Default::default()
        }
    }

    fn query(svc: &SearchService, h: &mut FakeHost, text: &str, redirects: &[Redirect]) -> Vec<SearchItem> {
        svc.query(h, text, redirects, &HashMap::new(), 0)
    }

    fn titles(items: &[SearchItem]) -> Vec<&str> {
        items.iter().map(|i| i.title.as_str()).collect()
    }

    fn by<'a>(items: &'a [SearchItem], title: &str) -> &'a SearchItem {
        items.iter().find(|i| i.title == title).unwrap_or_else(|| panic!("no hay «{title}» en {:?}", titles(items)))
    }

    #[test]
    fn busca_apps_instaladas_y_abiertas_marcando_las_que_estan_en_ejecucion() {
        let (svc, mut h) = (SearchService::new(), host());
        let items = query(&svc, &mut h, "s", &[]);
        // Steam (5 letras) se parece más a «s» que Spotify (7): sale antes; Notepad no coincide.
        assert_eq!(titles(&items), ["Steam", "Spotify"]);
        let spotify = by(&items, "Spotify");
        assert_eq!((spotify.kind, spotify.subtitle.as_str()), (SearchKind::Running, "En ejecución"));
        let steam = by(&items, "Steam");
        assert_eq!((steam.kind, steam.subtitle.as_str()), (SearchKind::App, "Aplicación"));
    }

    #[test]
    fn un_navegador_abierto_solo_por_una_app_web_abre_una_ventana_nueva_y_no_enfoca_la_otra() {
        // WhatsApp Web (una app de Chrome) tiene una ventana con el mismo ejecutable que «Google Chrome».
        let mut h = FakeHost {
            apps: vec![app("Google Chrome", "chrome", true), app("Microsoft Edge", "msedge", true), app("Firefox", "firefox", true)],
            open: vec![window(21, "chrome"), window(22, "msedge"), window(23, "firefox")],
            ..Default::default()
        };
        let svc = SearchService::new();
        for (text, title) in [("google", "Google Chrome"), ("edge", "Microsoft Edge"), ("fire", "Firefox")] {
            let items = query(&svc, &mut h, text, &[]);
            let item = by(&items, title);
            assert_eq!((item.kind, item.subtitle.as_str()), (SearchKind::App, "Aplicación"), "{title} no debe salir «En ejecución»");
            svc.open(&mut h, &item.id, false).unwrap();
        }
        assert!(h.focused.is_empty(), "no debe enfocar ninguna ventana: {:?}", h.focused);
        assert_eq!(h.launched.len(), 3);
        assert!(h.launched.iter().all(|(_, admin)| !admin));
    }

    #[test]
    fn con_ctrl_enter_un_navegador_abierto_tambien_abre_uno_nuevo_como_administrador() {
        let mut h = FakeHost { apps: vec![app("Google Chrome", "chrome", true)], open: vec![window(21, "chrome")], ..Default::default() };
        let svc = SearchService::new();
        let items = query(&svc, &mut h, "chrome", &[]);
        svc.open(&mut h, &items[0].id, true).unwrap();
        assert!(h.focused.is_empty());
        assert_eq!(h.launched, vec![(r"C:\Menu\Google Chrome.lnk".to_string(), true)]);
    }

    #[test]
    fn una_app_abierta_se_enfoca_y_una_cerrada_se_lanza() {
        let (svc, mut h) = (SearchService::new(), host());
        let items = query(&svc, &mut h, "s", &[]);
        assert_eq!(svc.open(&mut h, &by(&items, "Spotify").id, false).unwrap().as_deref(), Some("app:spotify:spotify"));
        assert_eq!(h.focused, vec![7]);
        assert!(h.launched.is_empty(), "no se lanza otra copia");
        svc.open(&mut h, &by(&items, "Steam").id, false).unwrap();
        assert_eq!(h.launched, vec![(r"C:\Menu\Steam.lnk".to_string(), false)]);
    }

    #[test]
    fn ctrl_enter_abre_como_administrador_tambien_una_ya_abierta() {
        let (svc, mut h) = (SearchService::new(), host());
        let items = query(&svc, &mut h, "s", &[]);
        svc.open(&mut h, &by(&items, "Steam").id, true).unwrap();
        svc.open(&mut h, &by(&items, "Spotify").id, true).unwrap();
        assert_eq!(h.launched, vec![(r"C:\Menu\Steam.lnk".into(), true), (r"C:\Programas\spotify.exe".into(), true)]);
        assert!(h.focused.is_empty());
    }

    #[test]
    fn si_la_ventana_ya_no_existe_se_abre_otra() {
        let (svc, mut h) = (SearchService::new(), host());
        let items = query(&svc, &mut h, "spo", &[]);
        h.focus_fails = true;
        svc.open(&mut h, &items[0].id, false).unwrap();
        assert_eq!(h.launched, vec![(r"C:\Programas\spotify.exe".to_string(), false)]);
    }

    const TEAMS: &str = r"shell:AppsFolder\MSTeams_8wekyb3d8bbwe!MSTeams";

    fn teams(running: bool) -> FakeHost {
        FakeHost {
            apps: vec![AppSuggestion { name: "Microsoft Teams".into(), exe: "ms-teams".into(), path: TEAMS.into(), running }],
            // La ventana de Teams vive en WindowsApps: ese `.exe` no se puede ejecutar directamente.
            open: if running {
                vec![OpenApp { handle: 11, exe: "ms-teams.exe".into(), path: r"C:\Program Files\WindowsApps\MSTeams_26260.1701.5139.3736_x64__8wekyb3d8bbwe\ms-teams.exe".into(), title: "Teams".into() }]
            } else {
                vec![]
            },
            ..Default::default()
        }
    }

    #[test]
    fn una_app_empaquetada_cerrada_se_abre_por_su_direccion_de_la_tienda() {
        let (svc, mut h) = (SearchService::new(), teams(false));
        let items = query(&svc, &mut h, "teams", &[]);
        assert_eq!(titles(&items), ["Microsoft Teams"]);
        assert!(svc.open(&mut h, &items[0].id, false).unwrap().is_some(), "no debe rechazarse por «ruta absoluta»");
        assert_eq!(h.launched, vec![(TEAMS.to_string(), false)]);
        // El logo se pide por esa misma dirección.
        assert_eq!(svc.icon_path(&items[0].id).as_deref(), Some(TEAMS));
    }

    #[test]
    fn una_app_empaquetada_abierta_se_enfoca_y_su_logo_sale_de_la_tienda_no_de_windowsapps() {
        let (svc, mut h) = (SearchService::new(), teams(true));
        let items = query(&svc, &mut h, "teams", &[]);
        assert_eq!(items[0].kind, SearchKind::Running);
        assert_eq!(svc.icon_path(&items[0].id).as_deref(), Some(TEAMS));
        svc.open(&mut h, &items[0].id, false).unwrap();
        assert_eq!(h.focused, vec![11]);
        assert!(h.launched.is_empty());
    }

    #[test]
    fn si_la_ventana_de_una_app_empaquetada_ya_no_existe_se_reabre_por_la_tienda() {
        let (svc, mut h) = (SearchService::new(), teams(true));
        let items = query(&svc, &mut h, "teams", &[]);
        h.focus_fails = true;
        svc.open(&mut h, &items[0].id, false).unwrap();
        assert_eq!(h.launched, vec![(TEAMS.to_string(), false)], "nunca el .exe de WindowsApps");
    }

    #[test]
    fn ctrl_enter_en_una_app_empaquetada_abierta_solo_la_enfoca_porque_no_se_puede_elevar() {
        let (svc, mut h) = (SearchService::new(), teams(true));
        let items = query(&svc, &mut h, "teams", &[]);
        svc.open(&mut h, &items[0].id, true).unwrap();
        assert_eq!(h.focused, vec![11]);
        assert!(h.launched.is_empty());
    }

    #[test]
    fn una_direccion_de_la_tienda_mal_formada_se_rechaza_aunque_la_ofrezca_el_host() {
        for bad in [r"shell:AppsFolder\a!b --x", r"shell:AppsFolder\..\x!y", r"shell:Desktop\cmd.exe"] {
            let svc = SearchService::new();
            let mut h = FakeHost { apps: vec![AppSuggestion { name: "Mala".into(), exe: "mala".into(), path: bad.into(), running: false }], ..Default::default() };
            let items = query(&svc, &mut h, "mala", &[]);
            assert!(svc.open(&mut h, &items[0].id, false).is_err(), "{bad}");
            assert!(h.launched.is_empty(), "{bad}");
        }
    }

    #[test]
    fn un_identificador_desconocido_no_hace_nada() {
        let (svc, mut h) = (SearchService::new(), host());
        query(&svc, &mut h, "s", &[]);
        for id in ["r9999", "", r"C:\Windows\System32\cmd.exe", "https://malo.com", "../x"] {
            assert_eq!(svc.open(&mut h, id, true).unwrap(), None, "{id}");
        }
        assert!(h.focused.is_empty() && h.launched.is_empty() && h.urls.is_empty());
        assert_eq!(svc.icon_path("r9999"), None);
    }

    #[test]
    fn una_redireccion_exacta_sale_primero_y_se_abre_en_el_navegador() {
        let (svc, mut h) = (SearchService::new(), host());
        let list = [redirect("claude", "https://claude.com", RedirectKind::Redirect)];
        let items = query(&svc, &mut h, "claude", &list);
        assert_eq!(items[0].kind, SearchKind::Redirect);
        assert_eq!((items[0].title.as_str(), items[0].subtitle.as_str()), ("Abrir claude.com", "https://claude.com"));
        assert_eq!(items[0].site.as_deref(), Some("https://claude.com"), "la vista pide el logo de la página con este origen");
        assert!(query(&svc, &mut h, "steam", &[]).iter().all(|i| i.site.is_none()), "las aplicaciones no tienen página");
        assert_eq!(svc.open(&mut h, &items[0].id, false).unwrap().as_deref(), Some("web:claude"));
        // ctrl+Enter en una dirección hace lo mismo que Enter.
        svc.open(&mut h, &items[0].id, true).unwrap();
        assert_eq!(h.urls, vec!["https://claude.com", "https://claude.com"]);
        assert!(h.launched.is_empty());
    }

    #[test]
    fn la_busqueda_web_codifica_el_texto_y_va_antes_que_las_apps() {
        let (svc, mut h) = (SearchService::new(), host());
        let list = [redirect("s", "https://www.google.com/search?q={q}", RedirectKind::Search)];
        let items = query(&svc, &mut h, "s gatos & perros", &list);
        assert_eq!(items[0].kind, SearchKind::WebSearch);
        assert_eq!((items[0].title.as_str(), items[0].subtitle.as_str()), ("Buscar «gatos & perros»", "Búsqueda web en google.com"));
        assert_eq!(items[0].site.as_deref(), Some("https://www.google.com"), "el logo es el del buscador, no lleva lo escrito");
        svc.open(&mut h, &items[0].id, false).unwrap();
        assert_eq!(h.urls, vec!["https://www.google.com/search?q=gatos%20%26%20perros"]);
    }

    #[test]
    fn las_palabras_con_espacios_funcionan_en_redirecciones_y_busquedas() {
        let (svc, mut h) = (SearchService::new(), host());
        let list = [
            redirect("mi banco", "https://banco.com", RedirectKind::Redirect),
            redirect("buscar en mapas", "https://maps.com/?q={q}", RedirectKind::Search),
        ];
        let items = query(&svc, &mut h, "mi  banco", &list);
        assert_eq!(items[0].kind, SearchKind::Redirect, "la redirección exacta sale primero");
        assert_eq!(svc.open(&mut h, &items[0].id, false).unwrap().as_deref(), Some("web:mi banco"));
        let items = query(&svc, &mut h, "buscar en mapas plaza mayor", &list);
        assert_eq!(items[0].kind, SearchKind::WebSearch);
        svc.open(&mut h, &items[0].id, false).unwrap();
        assert_eq!(h.urls, vec!["https://banco.com", "https://maps.com/?q=plaza%20mayor"]);
    }

    #[test]
    fn la_palabra_de_una_busqueda_sola_no_es_un_resultado() {
        let (svc, mut h) = (SearchService::new(), host());
        let list = [redirect("g", "https://www.google.com/search?q={q}", RedirectKind::Search)];
        assert!(query(&svc, &mut h, "g", &list).iter().all(|i| i.kind != SearchKind::WebSearch));
        assert!(query(&svc, &mut h, "g ", &list).iter().all(|i| i.kind != SearchKind::WebSearch));
    }

    #[test]
    fn el_uso_reordena_y_con_el_cuadro_vacio_salen_las_mas_usadas() {
        let (svc, mut h) = (SearchService::new(), host());
        let mut usage = HashMap::new();
        assert!(svc.query(&mut h, "", &[], &usage, 0).is_empty(), "sin historial no hay nada que sugerir");
        for _ in 0..3 {
            record_usage(&mut usage, "app:steam:steam", 0);
        }
        let items = svc.query(&mut h, "s", &[], &usage, 0);
        assert_eq!(titles(&items), ["Steam", "Spotify"], "Steam se abre más");
        assert_eq!(titles(&svc.query(&mut h, "", &[], &usage, 0)), ["Steam"]);
    }

    #[test]
    fn el_icono_se_pide_por_identificador_y_solo_de_programas() {
        let (svc, mut h) = (SearchService::new(), host());
        let list = [redirect("claude", "https://claude.com", RedirectKind::Redirect)];
        let apps = query(&svc, &mut h, "steam", &list);
        assert_eq!(svc.icon_path(&apps[0].id).as_deref(), Some(r"C:\Menu\Steam.lnk"));
        let running = query(&svc, &mut h, "spotify", &list);
        assert_eq!(svc.icon_path(&running[0].id).as_deref(), Some(r"C:\Programas\spotify.exe"));
        let web = query(&svc, &mut h, "claude", &list);
        assert_eq!(svc.icon_path(&web[0].id), None);
    }

    #[test]
    fn los_identificadores_de_una_consulta_anterior_siguen_valiendo_un_rato() {
        let (svc, mut h) = (SearchService::new(), host());
        let first = query(&svc, &mut h, "steam", &[]);
        let second = query(&svc, &mut h, "notepad", &[]);
        assert_ne!(first[0].id, second[0].id);
        assert!(svc.open(&mut h, &first[0].id, false).unwrap().is_some());
        // Pero no se acumulan sin límite.
        for _ in 0..REMEMBERED_IDS {
            query(&svc, &mut h, "s", &[]);
        }
        assert_eq!(svc.open(&mut h, &first[0].id, false).unwrap(), None);
    }

    #[test]
    fn solo_se_lanzan_programas_y_accesos_directos_validos() {
        let (svc, mut h) = (SearchService::new(), FakeHost { apps: vec![AppSuggestion { name: "Raro".into(), exe: "raro".into(), path: "cmd /c calc".into(), running: false }], ..Default::default() });
        let items = query(&svc, &mut h, "raro", &[]);
        assert!(svc.open(&mut h, &items[0].id, false).is_err());
        assert!(h.launched.is_empty());
    }

    #[test]
    fn dos_apps_con_el_mismo_ejecutable_tienen_historial_aparte() {
        let (svc, mut h) = (SearchService::new(), FakeHost { apps: vec![app("Discord", "update", false), app("Slack", "update", false)], ..Default::default() });
        let items = query(&svc, &mut h, "", &[]);
        assert!(items.is_empty());
        let keys: Vec<_> = ["discord", "slack"].iter().map(|q| {
            let items = query(&svc, &mut h, q, &[]);
            svc.open(&mut h, &items[0].id, false).unwrap().unwrap()
        }).collect();
        assert_eq!(keys, ["app:update:discord", "app:update:slack"]);
    }

    // --- Configuración de Windows -----------------------------------------------------------------------------------

    #[test]
    fn la_configuracion_de_windows_se_encuentra_por_su_nombre_y_por_otras_palabras() {
        let (svc, mut h) = (SearchService::new(), host());
        for (text, title) in [("configuracion", "Configuración"), ("ajustes", "Configuración"), ("settings", "Configuración"), ("privacidad", "Privacidad y seguridad"), ("barra", "Barra de tareas"), ("aplicaciones", "Aplicaciones instaladas")] {
            let items = query(&svc, &mut h, text, &[]);
            let found = by(&items, title);
            assert_eq!((found.kind, found.subtitle.as_str()), (SearchKind::App, "Configuración de Windows"), "{text}");
        }
        // «Configuración» es lo primero que sale al escribir su nombre.
        assert_eq!(query(&svc, &mut h, "configuracion", &[])[0].title, "Configuración");
    }

    #[test]
    fn con_una_sola_letra_mandan_las_apps_y_no_salen_paginas_de_la_configuracion() {
        let (svc, mut h) = (SearchService::new(), host());
        assert_eq!(titles(&query(&svc, &mut h, "s", &[])), ["Steam", "Spotify"]);
        assert!(query(&svc, &mut h, "", &[]).is_empty());
    }

    #[test]
    fn una_pagina_de_la_configuracion_abre_su_direccion_sin_elevar_y_lleva_el_icono_de_la_app() {
        let (svc, mut h) = (SearchService::new(), host());
        let items = query(&svc, &mut h, "barra de tareas", &[]);
        let page = by(&items, "Barra de tareas");
        assert_eq!(svc.icon_path(&page.id).as_deref(), Some(windows_settings::ICON_ADDRESS));
        assert_eq!(svc.open(&mut h, &page.id, false).unwrap().as_deref(), Some("setting:ms-settings:taskbar"));
        // Con ctrl+Enter también se abre normal: la Configuración no se puede elevar.
        svc.open(&mut h, &page.id, true).unwrap();
        assert_eq!(h.launched, vec![("ms-settings:taskbar".to_string(), false), ("ms-settings:taskbar".to_string(), false)]);
        assert!(h.focused.is_empty() && h.urls.is_empty());
    }

    #[test]
    fn la_portada_de_la_configuracion_abre_ms_settings_a_secas() {
        let (svc, mut h) = (SearchService::new(), host());
        let items = query(&svc, &mut h, "configuracion", &[]);
        svc.open(&mut h, &items[0].id, false).unwrap();
        assert_eq!(h.launched, vec![("ms-settings:".to_string(), false)]);
    }

    #[test]
    fn una_pagina_usada_sale_con_el_cuadro_vacio() {
        let (svc, h) = (SearchService::new(), &mut host());
        let mut usage = HashMap::new();
        record_usage(&mut usage, "setting:ms-settings:privacy", 0);
        let items = svc.query(h, "", &[], &usage, 0);
        assert_eq!(titles(&items), ["Privacidad y seguridad"]);
    }
}
