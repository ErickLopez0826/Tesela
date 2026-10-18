//! Tareas del día: texto sencillo con una prioridad. El orden de la lista es el orden en que se hacen. Al marcarlas como
//! hechas se eliminan, y no se exportan.

use serde::{Deserialize, Serialize};

pub const MAX_TASKS: usize = 200;
pub const MAX_TASK_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub text: String,
    pub priority: Priority,
}

/// Valida y limpia la lista que llega de la interfaz: el texto sin saltos de línea ni espacios de más, sin tareas vacías ni
/// demasiado largas, sin identificadores repetidos y con un máximo de tareas. Conserva el orden.
pub fn validate(list: Vec<Task>) -> Result<Vec<Task>, String> {
    if list.len() > MAX_TASKS {
        return Err(format!("como máximo {MAX_TASKS} tareas"));
    }
    let mut out: Vec<Task> = Vec::with_capacity(list.len());
    for mut task in list {
        task.text = task.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if task.text.is_empty() {
            return Err("una tarea está vacía".into());
        }
        if task.text.chars().count() > MAX_TASK_CHARS {
            return Err(format!("una tarea es demasiado larga (máximo {MAX_TASK_CHARS} caracteres)"));
        }
        if task.id.trim().is_empty() {
            return Err("una tarea no tiene identificador".into());
        }
        if out.iter().any(|t| t.id == task.id) {
            return Err("hay dos tareas con el mismo identificador".into());
        }
        out.push(task);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, text: &str, priority: Priority) -> Task {
        Task { id: id.into(), text: text.into(), priority }
    }

    #[test]
    fn el_texto_se_limpia_y_el_orden_se_conserva() {
        let list = validate(vec![task("b", "  comprar\n  pan  ", Priority::Low), task("a", "llamar", Priority::High)]).unwrap();
        assert_eq!(list[0].text, "comprar pan");
        assert_eq!(list.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
    }

    #[test]
    fn se_rechazan_vacias_largas_sin_id_repetidas_y_demasiadas() {
        assert!(validate(vec![task("a", "   \n ", Priority::Low)]).is_err());
        assert!(validate(vec![task("a", &"x".repeat(MAX_TASK_CHARS + 1), Priority::Low)]).is_err());
        assert!(validate(vec![task("a", &"x".repeat(MAX_TASK_CHARS), Priority::Low)]).is_ok());
        assert!(validate(vec![task(" ", "algo", Priority::Low)]).is_err());
        assert!(validate(vec![task("a", "uno", Priority::Low), task("a", "dos", Priority::High)]).is_err());
        let many: Vec<_> = (0..=MAX_TASKS).map(|i| task(&i.to_string(), "t", Priority::Medium)).collect();
        assert!(validate(many).is_err());
        assert_eq!(validate(vec![]).unwrap(), vec![]);
    }

    #[test]
    fn la_prioridad_se_guarda_en_minusculas() {
        let json = serde_json::to_string(&task("a", "t", Priority::High)).unwrap();
        assert!(json.contains("\"priority\":\"high\""), "{json}");
        let back: Task = serde_json::from_str(r#"{"id":"a","text":"t","priority":"low"}"#).unwrap();
        assert_eq!(back.priority, Priority::Low);
        assert!(serde_json::from_str::<Task>(r#"{"id":"a","text":"t","priority":"urgente"}"#).is_err());
    }
}
