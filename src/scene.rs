use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A parsed scene file (.tscn).
#[derive(Debug, Clone)]
pub struct SceneFile {
    pub path: PathBuf,
    /// External resources: id -> (type, path)
    pub ext_resources: Vec<ExtResource>,
    /// Nodes in the scene tree
    pub nodes: Vec<SceneNode>,
    /// Signal connections defined in the scene
    pub connections: Vec<SignalConnection>,
}

#[derive(Debug, Clone)]
pub struct ExtResource {
    pub id: String,
    pub resource_type: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct SceneNode {
    #[allow(dead_code)] // Parsed from scene file, used for debugging/future features
    pub name: String,
    pub node_type: String,
    pub parent: String,
    /// Script attached to this node (ext_resource id)
    pub script_id: Option<String>,
    /// Full node path from root
    pub node_path: String,
    /// Property names set on this node in the scene file
    pub properties: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SignalConnection {
    pub signal: String,
    pub from_node: String,
    pub to_node: String,
    pub method: String,
}

/// Parse all .tscn files under the project root.
pub fn parse_all_scenes(root: &Path) -> HashMap<PathBuf, SceneFile> {
    let mut scenes = HashMap::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "tscn") {
            if let Some(scene) = parse_tscn(path) {
                scenes.insert(path.to_path_buf(), scene);
            }
        }
    }
    scenes
}

/// Parse a single .tscn file.
fn parse_tscn(path: &Path) -> Option<SceneFile> {
    let content = std::fs::read_to_string(path).ok()?;

    let mut scene = SceneFile {
        path: path.to_path_buf(),
        ext_resources: Vec::new(),
        nodes: Vec::new(),
        connections: Vec::new(),
    };

    let mut i = 0;
    let lines: Vec<&str> = content.lines().collect();

    while i < lines.len() {
        let line = lines[i].trim();

        if line.starts_with("[ext_resource") {
            let attrs = parse_section_attrs(line);
            scene.ext_resources.push(ExtResource {
                id: attrs.get("id").cloned().unwrap_or_default(),
                resource_type: attrs.get("type").cloned().unwrap_or_default(),
                path: attrs.get("path").cloned().unwrap_or_default(),
            });
        } else if line.starts_with("[node") {
            let attrs = parse_section_attrs(line);
            let name = attrs.get("name").cloned().unwrap_or_default();
            let node_type = attrs.get("type").cloned().unwrap_or_default();
            let parent = attrs.get("parent").cloned().unwrap_or_default();

            // Check following lines for script assignment and properties
            let mut script_id = None;
            let mut properties = Vec::new();
            let mut j = i + 1;
            while j < lines.len() && !lines[j].starts_with('[') {
                let prop_line = lines[j].trim();
                if prop_line.starts_with("script") {
                    if let Some(id) = extract_ext_resource_id(prop_line) {
                        script_id = Some(id);
                    }
                } else if let Some(eq_pos) = prop_line.find(" = ") {
                    let prop_name = prop_line[..eq_pos].trim().to_string();
                    if !prop_name.is_empty() && prop_name != "script" {
                        properties.push(prop_name);
                    }
                }
                j += 1;
            }

            let node_path = build_node_path(&parent, &name);
            scene.nodes.push(SceneNode {
                name,
                node_type,
                parent,
                script_id,
                node_path,
                properties,
            });
        } else if line.starts_with("[connection") {
            let attrs = parse_section_attrs(line);
            scene.connections.push(SignalConnection {
                signal: attrs.get("signal").cloned().unwrap_or_default(),
                from_node: attrs.get("from").cloned().unwrap_or_default(),
                to_node: attrs.get("to").cloned().unwrap_or_default(),
                method: attrs.get("method").cloned().unwrap_or_default(),
            });
        }

        i += 1;
    }

    Some(scene)
}

/// Parse key=value attributes from a .tscn section header like
/// `[node name="Foo" type="Node2D" parent="."]`
fn parse_section_attrs(line: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();

    // Strip the brackets
    let inner = line.trim_start_matches('[').trim_end_matches(']').trim();

    // Skip the section keyword (ext_resource, node, connection, etc.)
    let after_keyword = if let Some(pos) = inner.find(' ') {
        &inner[pos + 1..]
    } else {
        return attrs;
    };

    // Simple state machine to parse key="value" pairs
    let chars: Vec<char> = after_keyword.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Skip whitespace
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        // Read key
        let key_start = i;
        while i < chars.len() && chars[i] != '=' {
            i += 1;
        }
        let key: String = chars[key_start..i].iter().collect();
        let key = key.trim().to_string();

        if i >= chars.len() {
            break;
        }
        i += 1; // skip '='

        // Read value
        if i < chars.len() && chars[i] == '"' {
            i += 1; // skip opening quote
            let val_start = i;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            let value: String = chars[val_start..i].iter().collect();
            attrs.insert(key, value);
            if i < chars.len() {
                i += 1; // skip closing quote
            }
        } else {
            // Unquoted value (rare in .tscn but handle it)
            let val_start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            let value: String = chars[val_start..i].iter().collect();
            attrs.insert(key, value);
        }
    }

    attrs
}

/// Extract the ExtResource ID from a property line like:
/// `script = ExtResource("1_abc")`
fn extract_ext_resource_id(line: &str) -> Option<String> {
    let start = line.find("ExtResource(")?;
    let after = &line[start + "ExtResource(".len()..];
    let end = after.find(')')?;
    let id = after[..end].trim().trim_matches('"');
    Some(id.to_string())
}

/// Build a full node path from parent path and node name.
fn build_node_path(parent: &str, name: &str) -> String {
    if parent.is_empty() || parent == "." {
        name.to_string()
    } else {
        format!("{}/{}", parent, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("test_scene.tscn")
    }

    #[test]
    fn parse_ext_resources() {
        let scene = parse_tscn(&fixture_path()).unwrap();
        assert_eq!(scene.ext_resources.len(), 1);
        assert_eq!(scene.ext_resources[0].resource_type, "Script");
        assert_eq!(scene.ext_resources[0].path, "res://scripts/Player.gd");
        assert_eq!(scene.ext_resources[0].id, "1_abc");
    }

    #[test]
    fn parse_nodes() {
        let scene = parse_tscn(&fixture_path()).unwrap();
        assert_eq!(scene.nodes.len(), 4);
        assert_eq!(scene.nodes[0].name, "Root");
        assert_eq!(scene.nodes[0].node_type, "Node3D");
        assert_eq!(scene.nodes[1].name, "Player");
        assert_eq!(scene.nodes[1].node_type, "CharacterBody3D");
        assert_eq!(scene.nodes[1].parent, ".");
    }

    #[test]
    fn parse_script_attachment() {
        let scene = parse_tscn(&fixture_path()).unwrap();
        let player = &scene.nodes[1];
        assert_eq!(player.script_id.as_deref(), Some("1_abc"));
    }

    #[test]
    fn parse_connections() {
        let scene = parse_tscn(&fixture_path()).unwrap();
        assert_eq!(scene.connections.len(), 1);
        assert_eq!(scene.connections[0].signal, "body_entered");
        assert_eq!(scene.connections[0].from_node, "Player");
        assert_eq!(scene.connections[0].to_node, ".");
        assert_eq!(scene.connections[0].method, "_on_player_body_entered");
    }

    #[test]
    fn parse_node_paths() {
        let scene = parse_tscn(&fixture_path()).unwrap();
        assert_eq!(scene.nodes[2].node_path, "Player/CollisionShape");
        assert_eq!(scene.nodes[3].node_path, "Player/Camera");
    }

    #[test]
    fn extract_ext_resource_id_works() {
        assert_eq!(
            extract_ext_resource_id("script = ExtResource(\"1_abc\")"),
            Some("1_abc".to_string())
        );
    }

    #[test]
    fn build_node_path_root() {
        assert_eq!(build_node_path("", "Root"), "Root");
        assert_eq!(build_node_path(".", "Player"), "Player");
    }

    #[test]
    fn build_node_path_nested() {
        assert_eq!(build_node_path("Player", "Camera"), "Player/Camera");
    }
}
