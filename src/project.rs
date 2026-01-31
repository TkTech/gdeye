use std::path::Path;

use indexmap::IndexMap;

/// Information extracted from a Godot project.godot file.
#[derive(Default, Debug, Clone)]
pub struct ProjectInfo {
    /// Autoload singletons: name -> script path (res:// relative)
    /// IndexMap preserves insertion order, which reflects the loading order in Godot.
    pub autoloads: IndexMap<String, String>,
    /// Project name
    pub name: String,
    /// Input action names defined in the [input] section.
    pub input_actions: Vec<String>,
}

/// Parse a project.godot file from the given project root directory.
pub fn parse_project(root: &Path) -> ProjectInfo {
    let path = root.join("project.godot");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return ProjectInfo::default(),
    };

    let mut info = ProjectInfo::default();
    let mut current_section = String::new();

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('[') && line.ends_with(']') {
            current_section = line[1..line.len() - 1].to_string();
            continue;
        }

        if line.is_empty() || line.starts_with(';') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            match current_section.as_str() {
                "application" => {
                    if key == "config/name" {
                        info.name = unquote(value);
                    }
                }
                "autoload" => {
                    // Format: name="*res://path/to/script.gd"
                    let script_path = unquote(value);
                    let script_path = script_path.trim_start_matches('*');
                    info.autoloads
                        .insert(key.to_string(), script_path.to_string());
                }
                "input" => {
                    info.input_actions.push(key.to_string());
                }
                _ => {}
            }
        }
    }

    info
}

/// Remove surrounding quotes from a Godot config value.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
    }

    #[test]
    fn parse_project_name() {
        let info = parse_project(&fixtures_dir());
        assert_eq!(info.name, "TestProject");
    }

    #[test]
    fn parse_autoloads() {
        let info = parse_project(&fixtures_dir());
        assert_eq!(info.autoloads.len(), 2);
        assert_eq!(
            info.autoloads.get("GameManager").unwrap(),
            "res://scripts/GameManager.gd"
        );
        assert_eq!(
            info.autoloads.get("EventBus").unwrap(),
            "res://scripts/EventBus.gd"
        );
    }

    #[test]
    fn parse_input_actions() {
        let info = parse_project(&fixtures_dir());
        assert!(info.input_actions.contains(&"move_left".to_string()));
        assert!(info.input_actions.contains(&"move_right".to_string()));
        assert!(info.input_actions.contains(&"jump".to_string()));
    }

    #[test]
    fn unquote_double_quotes() {
        assert_eq!(unquote("\"hello\""), "hello");
    }

    #[test]
    fn unquote_no_quotes() {
        assert_eq!(unquote("hello"), "hello");
    }
}
