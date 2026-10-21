//! Buscador de aplicaciones (§18): redirecciones y búsquedas web configurables, coincidencia, uso reciente y orden.
//! Todo puro: no sabe de Windows ni de la interfaz.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Cuántas redirecciones y búsquedas web se admiten.
pub const MAX_REDIRECTS: usize = 100;
/// Cuántos resultados devuelve una consulta como máximo (la interfaz enseña unos 8 y desplaza el resto).
pub const MAX_RESULTS: usize = 30;
/// Cuántos resultados se muestran con el cuadro vacío (los más usados).
pub const EMPTY_QUERY_RESULTS: usize = 8;
/// Cuántas entradas guarda el historial de uso.
pub const MAX_USAGE_ENTRIES: usize = 500;
/// Cada cuántos días el peso de un uso baja a la mitad.
pub const USAGE_HALF_LIFE_DAYS: f64 = 30.0;
/// Longitud máxima de la dirección de una redirección.
const MAX_URL_LEN: usize = 2000;

const SECS_PER_DAY: f64 = 86_400.0;
/// Marca de la dirección que se sustituye por lo escrito.
const QUERY_MARK: &str = "{q}";

// ---------------------------------------------------------------------------------------------------------------------
// Redirecciones y búsquedas web (§18.6)
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedirectKind {
    /// Abre la dirección tal cual.
    Redirect,
    /// La dirección lleva `{q}`, que se sustituye por el texto escrito tras la palabra.
    Search,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Redirect {
    /// Lo que se escribe para activarla.
    pub word: String,
    /// A dónde lleva (`http://` o `https://`).
    pub url: String,
    pub kind: RedirectKind,
}

/// ¿Es una dirección web que se puede abrir? Solo `http://` y `https://`, con servidor y sin espacios ni caracteres de control.
pub fn is_http_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let rest = match lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://")) {
        Some(rest) => rest,
        None => return false,
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    !host.is_empty() && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Valida y limpia la lista que llega de la interfaz.
pub fn validate_redirects(list: Vec<Redirect>) -> Result<Vec<Redirect>, String> {
    if list.len() > MAX_REDIRECTS {
        return Err(format!("como máximo {MAX_REDIRECTS} redirecciones y búsquedas"));
    }
    let mut out: Vec<Redirect> = Vec::new();
    for mut r in list {
        // La palabra puede llevar espacios («mi banco»); varios seguidos se dejan en uno.
        r.word = r.word.split_whitespace().collect::<Vec<_>>().join(" ");
        r.url = r.url.trim().to_string();
        if r.word.is_empty() {
            return Err("falta la palabra de una redirección".into());
        }
        if r.word.chars().any(char::is_control) {
            return Err(format!("la palabra «{}» lleva caracteres no permitidos", r.word));
        }
        if out.iter().any(|o| norm(&o.word) == norm(&r.word)) {
            return Err(format!("la palabra «{}» está repetida", r.word));
        }
        if r.url.len() > MAX_URL_LEN || !is_http_url(&r.url) {
            return Err(format!("la dirección de «{}» debe empezar por http:// o https://", r.word));
        }
        let marks = r.url.matches(QUERY_MARK).count();
        match r.kind {
            RedirectKind::Redirect if marks > 0 => {
                return Err(format!("«{}» es una redirección: la dirección no puede llevar {QUERY_MARK}", r.word));
            }
            RedirectKind::Search if marks == 0 => {
                return Err(format!("«{}» es una búsqueda: la dirección debe llevar {QUERY_MARK}", r.word));
            }
            RedirectKind::Search => {
                // Lo escrito nunca puede cambiar el servidor: el servidor debe estar completo antes de la primera marca.
                let before = r.url.split(QUERY_MARK).next().unwrap_or("");
                let after_scheme = before.split_once("://").map(|(_, rest)| rest).unwrap_or("");
                if !after_scheme.contains(['/', '?', '#']) {
                    return Err(format!("en «{}», {QUERY_MARK} debe ir después del servidor (por ejemplo https://sitio.com/buscar?q={QUERY_MARK})", r.word));
                }
            }
            RedirectKind::Redirect => {}
        }
        out.push(r);
    }
    Ok(out)
}

/// Codifica un texto para insertarlo en una dirección web: todo menos letras, cifras y `-._~` se escribe como `%XX` (UTF-8).
pub fn encode_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Dirección final de una búsqueda: la plantilla con `{q}` sustituido por el texto codificado.
pub fn build_url(template: &str, text: &str) -> String {
    template.replace(QUERY_MARK, &encode_query(text))
}

/// Una búsqueda web pedida con «palabra texto».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearch {
    pub word: String,
    pub text: String,
    pub url: String,
}

/// ¿Lo escrito es «palabra-de-búsqueda texto»? (`g gatos`, o `mi buscador gatos` si la palabra lleva espacios).
/// Sin texto tras la palabra no hay búsqueda. Si dos palabras encajan, gana la más larga.
pub fn parse_web_search(input: &str, redirects: &[Redirect]) -> Option<WebSearch> {
    let input = input.trim();
    let mut best: Option<WebSearch> = None;
    // La palabra es lo que hay antes de algún espacio de lo escrito y el texto, lo que hay después.
    for (at, c) in input.char_indices().filter(|(_, c)| c.is_whitespace()) {
        let (word, text) = (&input[..at], input[at + c.len_utf8()..].trim());
        if text.is_empty() {
            continue;
        }
        let Some(r) = redirects.iter().find(|r| r.kind == RedirectKind::Search && norm(&r.word) == norm(word)) else { continue };
        if best.as_ref().is_none_or(|b| r.word.len() > b.word.len()) {
            best = Some(WebSearch { word: r.word.clone(), text: text.to_string(), url: build_url(&r.url, text) });
        }
    }
    best
}

/// Origen de una dirección web (`https://www.claude.com/chat?x=1` → `https://www.claude.com`): de él sale el logo de la página.
/// `None` si no es una dirección `http(s)` válida.
pub fn origin_of(url: &str) -> Option<String> {
    if !is_http_url(url) {
        return None;
    }
    let (scheme, rest) = url.split_once("://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    // Sin usuario ni contraseña: solo el servidor (y el puerto, si lo lleva).
    let host = host.rsplit('@').next()?;
    (!host.is_empty()).then(|| format!("{}://{host}", scheme.to_ascii_lowercase()))
}

/// Nombre del servidor de una dirección, para mostrarla («https://www.claude.com/x» → «claude.com»).
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

// ---------------------------------------------------------------------------------------------------------------------
// Coincidencia (§18.7)
// ---------------------------------------------------------------------------------------------------------------------

/// Navegadores (nombre del ejecutable sin `.exe`, en minúsculas). Un navegador abierto casi nunca está «como navegador»: puede
/// tener solo una app web (WhatsApp Web, YouTube Music…) que comparte su ejecutable. Por eso, al elegir un navegador en el
/// buscador, **siempre se abre una ventana nueva** en lugar de enfocar la que ya hubiera.
///
/// Se añade `explorer`: el Explorador de archivos comparte ejecutable con el escritorio y con cualquier carpeta abierta (o
/// escondida en otro workspace), así que «enfocar» una de ellas no abría nada visible. Elegirlo abre siempre una ventana nueva.
const BROWSERS: &[&str] = &[
    "chrome", "msedge", "firefox", "brave", "opera", "opera_gx", "operagx", "vivaldi", "chromium", "iexplore", "waterfox", "librewolf", "floorp", "zen",
    "thorium", "arc", "yandex", "tor", "palemoon", "maxthon", "epiphany", "explorer",
];

/// ¿Debe abrir una ventana nueva (navegadores y Explorador de archivos) aunque ya haya otra con el mismo ejecutable?
pub fn opens_new_window(exe: &str) -> bool {
    let stem = exe.trim().to_lowercase();
    let stem = stem.trim_end_matches(".exe");
    BROWSERS.contains(&stem)
}

/// Minúsculas y sin tildes, para que «camara» encuentre «Cámara».
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

/// Como `fold`, y además con los espacios seguidos reducidos a uno: para comparar palabras que llevan espacios.
pub fn norm(s: &str) -> String {
    fold(&s.split_whitespace().collect::<Vec<_>>().join(" "))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Las letras aparecen en orden, pero separadas.
    Fuzzy = 1,
    /// Una palabra del nombre empieza por lo escrito.
    WordPrefix = 2,
    /// El nombre empieza por lo escrito.
    Prefix = 3,
}

/// Cómo coincide lo escrito con un nombre, y con qué cercanía (0 a 10: más alta cuanto más se parece la longitud).
pub fn match_tier(query: &str, name: &str) -> Option<(Tier, f64)> {
    let q = fold(query.trim());
    if q.is_empty() {
        return None;
    }
    let n = fold(name);
    let closeness = 10.0 * (q.chars().count() as f64 / n.chars().count().max(1) as f64).min(1.0);
    if n.starts_with(&q) {
        return Some((Tier::Prefix, closeness));
    }
    let mut from = 0;
    while let Some(i) = n[from..].find(&q) {
        let at = from + i;
        if n[..at].chars().next_back().is_some_and(|c| !c.is_alphanumeric()) {
            return Some((Tier::WordPrefix, closeness));
        }
        from = at + n[at..].chars().next().map_or(1, char::len_utf8);
    }
    let mut letters = q.chars().filter(|c| !c.is_whitespace());
    let mut wanted = letters.next();
    for c in n.chars() {
        if wanted == Some(c) {
            wanted = letters.next();
        }
    }
    wanted.is_none().then_some((Tier::Fuzzy, closeness))
}

// ---------------------------------------------------------------------------------------------------------------------
// Uso reciente (§18.7)
// ---------------------------------------------------------------------------------------------------------------------

/// Cuánto se ha abierto algo: un peso que envejece con el tiempo. `weight` vale lo que valía en `last_used`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UsageEntry {
    pub weight: f64,
    /// Segundos desde 1970 de la última vez que se abrió.
    pub last_used: u64,
}

fn decay(secs: u64) -> f64 {
    0.5f64.powf(secs as f64 / SECS_PER_DAY / USAGE_HALF_LIFE_DAYS)
}

impl UsageEntry {
    /// Peso a día de hoy, con el envejecimiento aplicado.
    pub fn current_weight(&self, now: u64) -> f64 {
        self.weight * decay(now.saturating_sub(self.last_used))
    }
}

/// Puntos que añade el uso: logarítmico y con tope, así reordena dentro de cada nivel de coincidencia pero no lo salta.
pub fn usage_bonus(entry: Option<&UsageEntry>, now: u64) -> f64 {
    entry.map_or(0.0, |e| ((1.0 + e.current_weight(now)).ln() * 30.0).min(80.0))
}

/// Anota que se abrió algo. Si el historial se llena, se olvida lo que menos pesa.
pub fn record_usage(usage: &mut HashMap<String, UsageEntry>, key: &str, now: u64) {
    let weight = usage.get(key).map_or(0.0, |e| e.current_weight(now)) + 1.0;
    usage.insert(key.to_string(), UsageEntry { weight, last_used: now });
    if usage.len() > MAX_USAGE_ENTRIES {
        let mut by_weight: Vec<(String, f64)> = usage.iter().map(|(k, e)| (k.clone(), e.current_weight(now))).collect();
        by_weight.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        for (k, _) in by_weight.into_iter().take(usage.len() - MAX_USAGE_ENTRIES) {
            usage.remove(&k);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Orden de los resultados (§18.7)
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    /// Aplicación instalada que no está abierta.
    App,
    /// Aplicación abierta ahora: al elegirla se enfoca.
    Running,
    Redirect,
    WebSearch,
}

/// Un resultado tal como lo ve la interfaz. El `id` es opaco: solo el núcleo sabe a qué corresponde (§18.10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchItem {
    pub id: String,
    pub kind: SearchKind,
    pub title: String,
    pub subtitle: String,
    /// Origen de la página (`https://claude.ai`) de una redirección o búsqueda web: la vista pide su logo. `None` en las aplicaciones.
    pub site: Option<String>,
}

/// Algo que puede salir como resultado, antes de puntuarlo.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Clave estable para el historial de uso (`app:spotify`, `web:claude`).
    pub key: String,
    pub kind: SearchKind,
    pub title: String,
    pub subtitle: String,
    /// Origen de la página, si es una redirección o una búsqueda web.
    pub site: Option<String>,
    /// Texto contra el que se compara lo escrito.
    pub match_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ranked {
    pub candidate: Candidate,
    pub score: f64,
}

/// Puntos de una redirección cuya palabra es exactamente lo escrito: sube por encima de todo lo demás.
pub const EXACT_REDIRECT_SCORE: f64 = 500.0;
/// Puntos de una búsqueda web pedida con «palabra texto»: por debajo de una redirección exacta y por encima de cualquier coincidencia.
pub const WEB_SEARCH_SCORE: f64 = 450.0;

/// Puntúa y ordena. Sin coincidencia no aparece. Con el cuadro vacío salen los más usados.
/// Orden: puntuación, luego las abiertas y luego el alfabeto.
pub fn rank(query: &str, candidates: Vec<Candidate>, usage: &HashMap<String, UsageEntry>, now: u64) -> Vec<Ranked> {
    let query = query.trim();
    let mut out: Vec<Ranked> = Vec::new();
    for candidate in candidates {
        let bonus = usage_bonus(usage.get(&candidate.key), now);
        let score = if query.is_empty() {
            if bonus <= 0.0 {
                continue;
            }
            bonus
        } else if candidate.kind == SearchKind::Redirect && norm(&candidate.match_text) == norm(query) {
            EXACT_REDIRECT_SCORE
        } else {
            match match_tier(query, &candidate.match_text) {
                Some((tier, closeness)) => tier as u8 as f64 * 100.0 + closeness + bonus,
                None => continue,
            }
        };
        out.push(Ranked { candidate, score });
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| (b.candidate.kind == SearchKind::Running).cmp(&(a.candidate.kind == SearchKind::Running)))
            .then_with(|| fold(&a.candidate.title).cmp(&fold(&b.candidate.title)))
    });
    out.truncate(if query.is_empty() { EMPTY_QUERY_RESULTS } else { MAX_RESULTS });
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn los_navegadores_siempre_abren_ventana_nueva_y_el_resto_no() {
        for exe in ["chrome", "Chrome.exe", "msedge", "firefox", "brave", "opera", "vivaldi", "chromium", "iexplore", "waterfox", "librewolf", "explorer", "Explorer.exe"] {
            assert!(super::opens_new_window(exe), "{exe}");
        }
        for exe in ["spotify", "code", "notepad", "chrome_proxy", "discord", "", "chromedriver"] {
            assert!(!super::opens_new_window(exe), "{exe}");
        }
    }

    use super::*;

    fn redirect(word: &str, url: &str, kind: RedirectKind) -> Redirect {
        Redirect { word: word.into(), url: url.into(), kind }
    }

    fn cand(key: &str, kind: SearchKind, title: &str) -> Candidate {
        Candidate { key: key.into(), kind, title: title.into(), subtitle: String::new(), match_text: title.into(), site: None }
    }

    const DAY: u64 = 86_400;

    // --- validación -------------------------------------------------------------------------------------------------

    #[test]
    fn una_redireccion_y_una_busqueda_validas_se_aceptan_y_se_limpian() {
        let list = validate_redirects(vec![
            redirect("  claude ", " https://claude.com ", RedirectKind::Redirect),
            redirect("g", "https://www.google.com/search?q={q}", RedirectKind::Search),
        ])
        .unwrap();
        assert_eq!((list[0].word.as_str(), list[0].url.as_str()), ("claude", "https://claude.com"));
        assert_eq!(list.len(), 2);
        assert_eq!(validate_redirects(vec![]).unwrap(), vec![]);
    }

    #[test]
    fn solo_se_admiten_direcciones_http_y_https() {
        for bad in ["file:///C:/x", "javascript:alert(1)", "ftp://x.com", "claude.com", "data:text/html,hola", "https://", "https:///x", "http://a b.com", "mailto:a@b.c"] {
            assert!(validate_redirects(vec![redirect("x", bad, RedirectKind::Redirect)]).is_err(), "{bad}");
        }
        assert!(validate_redirects(vec![redirect("x", "HTTPS://Claude.com", RedirectKind::Redirect)]).is_ok());
        assert!(validate_redirects(vec![redirect("x", "http://localhost:3000/a?b=1#c", RedirectKind::Redirect)]).is_ok());
    }

    #[test]
    fn palabras_vacias_o_repetidas_se_rechazan() {
        let ok = |w: &str| redirect(w, "https://a.com", RedirectKind::Redirect);
        assert!(validate_redirects(vec![ok("  ")]).is_err());
        assert!(validate_redirects(vec![ok("a\u{7}b")]).is_err(), "sin caracteres de control");
        assert!(validate_redirects(vec![ok("Claude"), ok("claude")]).is_err(), "sin distinguir mayúsculas");
        assert!(validate_redirects(vec![ok("camara"), ok("Cámara")]).is_err(), "ni tildes");
        assert!(validate_redirects(vec![ok("mi banco"), ok("MI   Banco")]).is_err(), "ni espacios de más");
    }

    #[test]
    fn la_palabra_puede_llevar_espacios_y_se_limpian_los_sobrantes() {
        let list = validate_redirects(vec![
            redirect("  mi   banco ", "https://banco.com", RedirectKind::Redirect),
            redirect("buscar en mapas", "https://maps.com/?q={q}", RedirectKind::Search),
        ])
        .unwrap();
        assert_eq!(list[0].word, "mi banco");
        assert_eq!(list[1].word, "buscar en mapas");
    }

    #[test]
    fn la_marca_q_solo_va_en_las_busquedas_y_despues_del_servidor() {
        assert!(validate_redirects(vec![redirect("g", "https://google.com/search?q={q}", RedirectKind::Search)]).is_ok());
        assert!(validate_redirects(vec![redirect("g", "https://google.com", RedirectKind::Search)]).is_err(), "búsqueda sin marca");
        assert!(validate_redirects(vec![redirect("g", "https://google.com/?q={q}", RedirectKind::Redirect)]).is_err(), "redirección con marca");
        assert!(validate_redirects(vec![redirect("g", "https://{q}.com", RedirectKind::Search)]).is_err(), "lo escrito no puede cambiar el servidor");
        assert!(validate_redirects(vec![redirect("g", "https://google.com{q}", RedirectKind::Search)]).is_err());
    }

    #[test]
    fn hay_un_tope_de_entradas() {
        let many: Vec<_> = (0..=MAX_REDIRECTS).map(|i| redirect(&format!("w{i}"), "https://a.com", RedirectKind::Redirect)).collect();
        assert!(validate_redirects(many).is_err());
    }

    // --- direcciones ------------------------------------------------------------------------------------------------

    #[test]
    fn codificar_lo_escrito_cubre_tildes_espacios_y_simbolos() {
        assert_eq!(encode_query("gatos"), "gatos");
        assert_eq!(encode_query("gatos negros"), "gatos%20negros");
        assert_eq!(encode_query("camión"), "cami%C3%B3n");
        assert_eq!(encode_query("a&b=c#d?e/f"), "a%26b%3Dc%23d%3Fe%2Ff");
        assert_eq!(encode_query("100% ñ"), "100%25%20%C3%B1");
        assert_eq!(encode_query("a-b_c.d~e"), "a-b_c.d~e");
    }

    #[test]
    fn la_busqueda_sustituye_la_marca_por_el_texto_codificado() {
        assert_eq!(build_url("https://google.com/search?q={q}", "hola mundo"), "https://google.com/search?q=hola%20mundo");
        assert_eq!(build_url("https://x.com/{q}/{q}", "a b"), "https://x.com/a%20b/a%20b");
    }

    #[test]
    fn palabra_y_texto_se_reconocen_como_busqueda_web() {
        let list = vec![redirect("g", "https://google.com/search?q={q}", RedirectKind::Search), redirect("claude", "https://claude.com", RedirectKind::Redirect)];
        let s = parse_web_search("g gatos negros", &list).unwrap();
        assert_eq!((s.word.as_str(), s.text.as_str()), ("g", "gatos negros"));
        assert_eq!(s.url, "https://google.com/search?q=gatos%20negros");
        assert_eq!(parse_web_search("G  gatos ", &list).unwrap().text, "gatos", "sin distinguir mayúsculas y recortando");
        assert_eq!(parse_web_search("g", &list), None, "sin texto no hay búsqueda");
        assert_eq!(parse_web_search("g   ", &list), None);
        assert_eq!(parse_web_search("x gatos", &list), None, "palabra desconocida");
        assert_eq!(parse_web_search("claude hola", &list), None, "una redirección no es una búsqueda");
    }

    #[test]
    fn una_busqueda_con_espacios_en_la_palabra_se_reconoce_y_gana_la_mas_larga() {
        let list = vec![
            redirect("buscar", "https://corta.com/?q={q}", RedirectKind::Search),
            redirect("buscar en mapas", "https://maps.com/?q={q}", RedirectKind::Search),
        ];
        let s = parse_web_search("Buscar  EN mapas  plaza mayor", &list).unwrap();
        assert_eq!((s.word.as_str(), s.text.as_str()), ("buscar en mapas", "plaza mayor"));
        assert_eq!(s.url, "https://maps.com/?q=plaza%20mayor");
        // Con una palabra más corta que también encaja, el resto va como texto.
        let s = parse_web_search("buscar en casa", &list).unwrap();
        assert_eq!((s.word.as_str(), s.text.as_str()), ("buscar", "en casa"));
        assert_eq!(parse_web_search("buscar en mapas", &list).unwrap().word, "buscar", "sin texto tras la palabra larga, solo encaja la corta");
        assert_eq!(parse_web_search("buscar en mapas   ", &list).unwrap().text, "en mapas");
    }

    #[test]
    fn el_origen_es_el_esquema_y_el_servidor_y_nada_mas() {
        assert_eq!(origin_of("https://www.claude.com/chat?x=1#a").as_deref(), Some("https://www.claude.com"));
        assert_eq!(origin_of("HTTP://localhost:3000/a").as_deref(), Some("http://localhost:3000"));
        assert_eq!(origin_of("https://usuario:clave@sitio.com/x").as_deref(), Some("https://sitio.com"), "sin credenciales");
        assert_eq!(origin_of("https://gemini.google.com").as_deref(), Some("https://gemini.google.com"));
        for bad in ["file:///C:/x", "javascript:alert(1)", "claude.com", "https://", ""] {
            assert_eq!(origin_of(bad), None, "{bad}");
        }
    }

    #[test]
    fn el_servidor_se_muestra_sin_www() {
        assert_eq!(host_of("https://www.claude.com/chat?x=1"), "claude.com");
        assert_eq!(host_of("http://localhost:3000"), "localhost:3000");
    }

    // --- coincidencia -----------------------------------------------------------------------------------------------

    #[test]
    fn los_tres_niveles_de_coincidencia_se_ordenan() {
        let tier = |q: &str, n: &str| match_tier(q, n).map(|(t, _)| t);
        assert_eq!(tier("spo", "Spotify"), Some(Tier::Prefix));
        assert_eq!(tier("studio", "Visual Studio Code"), Some(Tier::WordPrefix));
        assert_eq!(tier("vsc", "Visual Studio Code"), Some(Tier::Fuzzy));
        assert_eq!(tier("zzz", "Spotify"), None);
        assert!(Tier::Prefix > Tier::WordPrefix && Tier::WordPrefix > Tier::Fuzzy);
    }

    #[test]
    fn la_coincidencia_ignora_mayusculas_y_tildes() {
        assert_eq!(match_tier("camara", "Cámara").map(|(t, _)| t), Some(Tier::Prefix));
        assert_eq!(match_tier("CÁMARA", "camara").map(|(t, _)| t), Some(Tier::Prefix));
        assert_eq!(match_tier("nino", "El Niño").map(|(t, _)| t), Some(Tier::WordPrefix));
    }

    #[test]
    fn una_palabra_a_mitad_de_otra_no_cuenta_como_palabra() {
        // «tify» está dentro de «Spotify» pero no empieza ninguna palabra: solo coincide de forma difusa.
        assert_eq!(match_tier("tify", "Spotify").map(|(t, _)| t), Some(Tier::Fuzzy));
    }

    #[test]
    fn el_texto_vacio_no_coincide_con_nada() {
        assert_eq!(match_tier("", "Spotify"), None);
        assert_eq!(match_tier("   ", "Spotify"), None);
    }

    #[test]
    fn mas_parecido_en_longitud_puntua_mas() {
        let (_, short) = match_tier("spotify", "Spotify").unwrap();
        let (_, long) = match_tier("spotify", "Spotify Premium Installer").unwrap();
        assert!(short > long);
    }

    // --- uso reciente -----------------------------------------------------------------------------------------------

    #[test]
    fn el_uso_envejece_a_la_mitad_cada_treinta_dias() {
        let e = UsageEntry { weight: 8.0, last_used: 0 };
        assert!((e.current_weight(0) - 8.0).abs() < 1e-9);
        assert!((e.current_weight(30 * DAY) - 4.0).abs() < 1e-6);
        assert!((e.current_weight(60 * DAY) - 2.0).abs() < 1e-6);
        assert!(usage_bonus(Some(&e), 0) > usage_bonus(Some(&e), 90 * DAY), "con el tiempo el bonus baja");
    }

    #[test]
    fn el_bonus_de_uso_tiene_tope_y_es_cero_sin_uso() {
        assert_eq!(usage_bonus(None, 0), 0.0);
        let huge = UsageEntry { weight: 1e9, last_used: 0 };
        assert!(usage_bonus(Some(&huge), 0) <= 80.0);
    }

    #[test]
    fn anotar_un_uso_suma_y_conserva_lo_anterior_envejecido() {
        let mut usage = HashMap::new();
        record_usage(&mut usage, "app:a", 0);
        record_usage(&mut usage, "app:a", 30 * DAY);
        let w = usage["app:a"].weight;
        assert!((w - 1.5).abs() < 1e-6, "1 envejecido a 0,5 + 1 nuevo, fue {w}");
        assert_eq!(usage["app:a"].last_used, 30 * DAY);
    }

    #[test]
    fn el_historial_se_poda_olvidando_lo_que_menos_pesa() {
        let mut usage = HashMap::new();
        record_usage(&mut usage, "app:favorita", 0);
        record_usage(&mut usage, "app:favorita", 0);
        for i in 0..MAX_USAGE_ENTRIES + 20 {
            record_usage(&mut usage, &format!("app:{i}"), 0);
        }
        assert_eq!(usage.len(), MAX_USAGE_ENTRIES);
        assert!(usage.contains_key("app:favorita"), "la más usada sobrevive");
    }

    // --- orden ------------------------------------------------------------------------------------------------------

    fn titles(r: &[Ranked]) -> Vec<&str> {
        r.iter().map(|r| r.candidate.title.as_str()).collect()
    }

    #[test]
    fn sin_coincidencia_no_aparece_y_los_niveles_mandan() {
        let list = vec![
            cand("app:a", SearchKind::App, "Visual Studio Code"),
            cand("app:b", SearchKind::App, "Studio One"),
            cand("app:c", SearchKind::App, "Notepad"),
            cand("app:d", SearchKind::App, "Calculadora"),
        ];
        let ranked = rank("studio", list, &HashMap::new(), 0);
        assert_eq!(titles(&ranked), ["Studio One", "Visual Studio Code"]);
    }

    #[test]
    fn la_coincidencia_difusa_va_la_ultima() {
        // «sistema de tu dispositivo» contiene s-t-u-d-i-o en orden, pero separadas: entra, detrás de las demás.
        let list = vec![cand("app:d", SearchKind::App, "Sistema de Tu Dispositivo"), cand("app:b", SearchKind::App, "Studio One")];
        assert_eq!(titles(&rank("studio", list, &HashMap::new(), 0)), ["Studio One", "Sistema de Tu Dispositivo"]);
    }

    #[test]
    fn el_uso_reciente_sube_un_resultado_dentro_de_su_nivel() {
        let mut usage = HashMap::new();
        for _ in 0..5 {
            record_usage(&mut usage, "app:code", 0);
        }
        let list = vec![cand("app:cmd", SearchKind::App, "Cmd"), cand("app:code", SearchKind::App, "Code")];
        let ranked = rank("c", list, &usage, 0);
        assert_eq!(titles(&ranked), ["Code", "Cmd"]);
    }

    #[test]
    fn el_uso_no_rescata_a_lo_que_no_coincide_ni_salta_de_nivel() {
        let mut usage = HashMap::new();
        for _ in 0..1000 {
            record_usage(&mut usage, "app:word", 0);
        }
        let list = vec![cand("app:word", SearchKind::App, "Word"), cand("app:cal", SearchKind::App, "Calculadora")];
        assert_eq!(titles(&rank("calc", list, &usage, 0)), ["Calculadora"], "Word no coincide: no aparece por muy usado que sea");

        // Una difusa muy usada no pasa por delante de una coincidencia por palabra sin uso.
        for _ in 0..1000 {
            record_usage(&mut usage, "app:fuzzy", 0);
        }
        let list = vec![cand("app:fuzzy", SearchKind::App, "Pxo Tool"), cand("app:palabra", SearchKind::App, "Word Pro Xo")];
        assert_eq!(titles(&rank("xo", list, &usage, 0)), ["Word Pro Xo", "Pxo Tool"]);
    }

    #[test]
    fn el_uso_antiguo_pesa_menos_que_el_reciente() {
        let mut usage = HashMap::new();
        for _ in 0..4 {
            record_usage(&mut usage, "app:vieja", 0);
        }
        record_usage(&mut usage, "app:nueva", 200 * DAY);
        record_usage(&mut usage, "app:nueva", 200 * DAY);
        let list = vec![cand("app:vieja", SearchKind::App, "Alfa"), cand("app:nueva", SearchKind::App, "Alba")];
        let ranked = rank("al", list, &usage, 200 * DAY);
        assert_eq!(titles(&ranked)[0], "Alba");
    }

    #[test]
    fn en_un_empate_van_primero_las_abiertas_y_luego_el_alfabeto() {
        let list = vec![cand("app:b", SearchKind::App, "Beta"), cand("running:b", SearchKind::Running, "Bravo"), cand("app:a", SearchKind::App, "Bar")];
        // Las tres empiezan por «b» y se parecen por igual en longitud al ser de 3 a 5 letras: se separan por closeness,
        // así que se comparan con un texto igual de largo.
        let list: Vec<_> = list.into_iter().map(|mut c| { c.match_text = "Bxxxx".into(); c }).collect();
        let ranked = rank("b", list, &HashMap::new(), 0);
        assert_eq!(titles(&ranked), ["Bravo", "Bar", "Beta"]);
    }

    #[test]
    fn la_redireccion_exacta_sube_a_primer_lugar() {
        let mut usage = HashMap::new();
        for _ in 0..50 {
            record_usage(&mut usage, "app:claude-desktop", 0);
        }
        let list = vec![
            Candidate { key: "web:claude".into(), kind: SearchKind::Redirect, title: "Abrir claude.com".into(), subtitle: String::new(), match_text: "claude".into(), site: None },
            cand("app:claude-desktop", SearchKind::App, "Claude Desktop"),
        ];
        let ranked = rank("claude", list.clone(), &usage, 0);
        assert_eq!(ranked[0].candidate.key, "web:claude");
        assert_eq!(ranked[0].score, EXACT_REDIRECT_SCORE);
        // Si no es exacta, la redirección compite como cualquier otra coincidencia.
        let ranked = rank("clau", list, &usage, 0);
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].candidate.key, "app:claude-desktop", "más usada y mismo nivel");
    }

    #[test]
    fn una_redireccion_con_espacios_se_reconoce_exacta_aunque_se_escriban_de_mas() {
        let c = Candidate { key: "web:mi banco".into(), kind: SearchKind::Redirect, title: "Abrir banco.com".into(), subtitle: String::new(), match_text: "mi banco".into(), site: None };
        let ranked = rank("  MI    banco ", vec![c], &HashMap::new(), 0);
        assert_eq!(ranked[0].score, EXACT_REDIRECT_SCORE);
    }

    #[test]
    fn la_busqueda_web_pedida_esta_por_debajo_de_la_redireccion_exacta_y_por_encima_de_las_apps() {
        assert!(EXACT_REDIRECT_SCORE > WEB_SEARCH_SCORE);
        let max_match = Tier::Prefix as u8 as f64 * 100.0 + 10.0 + 80.0;
        assert!(WEB_SEARCH_SCORE > max_match, "ninguna coincidencia normal la supera");
    }

    #[test]
    fn con_el_cuadro_vacio_salen_las_mas_usadas_y_solo_esas() {
        let mut usage = HashMap::new();
        for _ in 0..3 {
            record_usage(&mut usage, "app:uno", 0);
        }
        record_usage(&mut usage, "app:dos", 0);
        let list = vec![cand("app:dos", SearchKind::App, "Dos"), cand("app:uno", SearchKind::App, "Uno"), cand("app:tres", SearchKind::App, "Tres")];
        let ranked = rank("  ", list, &usage, 0);
        assert_eq!(titles(&ranked), ["Uno", "Dos"], "Tres nunca se abrió");
        assert!(rank("", vec![cand("app:a", SearchKind::App, "A")], &HashMap::new(), 0).is_empty());
    }

    #[test]
    fn con_el_cuadro_vacio_hay_un_tope_de_resultados() {
        let mut usage = HashMap::new();
        let list: Vec<_> = (0..20)
            .map(|i| {
                record_usage(&mut usage, &format!("app:{i}"), 0);
                cand(&format!("app:{i}"), SearchKind::App, &format!("App {i:02}"))
            })
            .collect();
        assert_eq!(rank("", list, &usage, 0).len(), EMPTY_QUERY_RESULTS);
    }
}
