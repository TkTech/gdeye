use std::collections::HashMap;
use std::io::Read;

use flate2::read::DeflateDecoder;
use serde::Deserialize;

use crate::error::GdEyeError;

// Include the build-generated registry of all bundled ClassDB versions.
include!(concat!(env!("OUT_DIR"), "/classdb_registry.rs"));

#[derive(Debug, Clone, Deserialize)]
struct RawApi {
    #[serde(default)]
    classes: Vec<RawClass>,
    #[serde(default)]
    builtin_classes: Vec<RawBuiltinClass>,
    #[serde(default)]
    utility_functions: Vec<RawUtilityFunction>,
    #[serde(default)]
    global_enums: Vec<RawEnum>,
    #[serde(default)]
    singletons: Vec<RawSingleton>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawClass {
    name: String,
    #[serde(default)]
    inherits: String,
    #[serde(default)]
    methods: Vec<RawMethod>,
    #[serde(default)]
    properties: Vec<RawProperty>,
    #[serde(default)]
    signals: Vec<RawSignal>,
    #[serde(default)]
    constants: Vec<RawConstant>,
    #[serde(default)]
    enums: Vec<RawEnum>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawBuiltinClass {
    name: String,
    #[serde(default)]
    methods: Vec<RawBuiltinMethod>,
    #[serde(default)]
    operators: Vec<RawOperator>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawOperator {
    name: String,
    #[serde(default)]
    right_type: String,
    #[serde(default)]
    return_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawMethod {
    name: String,
    #[serde(default)]
    is_static: bool,
    #[serde(default)]
    is_virtual: bool,
    #[serde(default)]
    return_type: String,
    #[serde(default)]
    arguments: Vec<RawArgument>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawBuiltinMethod {
    name: String,
    #[serde(default)]
    is_static: bool,
    #[serde(default)]
    return_type: String,
    #[serde(default)]
    arguments: Vec<RawArgument>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawProperty {
    name: String,
    #[serde(default, rename = "type")]
    prop_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSignal {
    name: String,
    #[serde(default)]
    arguments: Vec<RawArgument>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawConstant {
    name: String,
    #[serde(default)]
    value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
struct RawEnum {
    name: String,
    #[serde(default)]
    values: Vec<RawEnumValue>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawEnumValue {
    name: String,
    #[serde(default)]
    value: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct RawArgument {
    name: String,
    #[serde(default, rename = "type")]
    arg_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawUtilityFunction {
    name: String,
    #[serde(default)]
    is_vararg: bool,
    #[serde(default)]
    return_type: String,
    #[serde(default)]
    arguments: Vec<RawArgument>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSingleton {
    name: String,
    #[serde(default, rename = "type")]
    singleton_type: String,
}

/// Information about a method on a class or builtin type.
#[allow(dead_code)] // Fields from Godot API schema
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    pub is_static: bool,
    pub is_virtual: bool,
    pub return_type: String,
    pub arguments: Vec<ArgumentInfo>,
}

/// Information about a method argument.
#[allow(dead_code)] // Fields from Godot API schema
#[derive(Debug, Clone)]
pub struct ArgumentInfo {
    pub name: String,
    pub arg_type: String,
}

/// Information about a class property.
#[derive(Debug, Clone)]
pub struct PropertyInfo {
    pub name: String,
    pub prop_type: String,
}

/// Information about a signal.
#[derive(Debug, Clone)]
pub struct SignalInfo {
    pub name: String,
    pub arguments: Vec<ArgumentInfo>,
}

/// Information about a constant.
#[allow(dead_code)] // Fields from Godot API schema
#[derive(Debug, Clone)]
pub struct ConstantInfo {
    pub name: String,
    pub value: i64,
}

/// Information about an enum.
#[allow(dead_code)] // Fields from Godot API schema
#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub values: Vec<EnumValueInfo>,
}

/// A single enum value.
#[allow(dead_code)] // Fields from Godot API schema
#[derive(Debug, Clone)]
pub struct EnumValueInfo {
    pub name: String,
    pub value: i64,
}

/// Information about a Godot engine class.
#[allow(dead_code)] // Fields from Godot API schema
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub parent: String,
    pub methods: Vec<MethodInfo>,
    pub properties: Vec<PropertyInfo>,
    pub signals: Vec<SignalInfo>,
    pub constants: Vec<ConstantInfo>,
    pub enums: Vec<EnumInfo>,
}

/// Information about a builtin type (Array, Dictionary, Vector2, etc.)
#[derive(Debug, Clone)]
pub struct BuiltinClassInfo {
    pub name: String,
    pub methods: Vec<MethodInfo>,
    pub operators: Vec<OperatorInfo>,
}

/// Information about an operator on a builtin type.
#[derive(Debug, Clone)]
pub struct OperatorInfo {
    /// The operator symbol (e.g., "+", "*", "==")
    pub name: String,
    /// The right-hand operand type (empty for unary operators)
    pub right_type: String,
    /// The result type of the operation
    pub return_type: String,
}

/// Information about a global utility function (print, range, etc.)
#[allow(dead_code)] // Fields from Godot API schema
#[derive(Debug, Clone)]
pub struct UtilityFunctionInfo {
    pub name: String,
    pub is_vararg: bool,
    pub return_type: String,
    pub arguments: Vec<ArgumentInfo>,
}

/// Describes where the ClassDB data was loaded from.
#[derive(Debug, Clone)]
pub enum ClassDbSource {
    /// Bundled at build time from extension_api.json
    Bundled { version: String },
    /// Loaded at runtime from the user's Godot binary
    Runtime { version: String, path: String },
}

impl std::fmt::Display for ClassDbSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClassDbSource::Bundled { version } => write!(f, "bundled (Godot {})", version),
            ClassDbSource::Runtime { version, path } => {
                write!(f, "runtime (Godot {}, {})", version, path)
            }
        }
    }
}

/// The Godot class database, providing fast lookups for class hierarchy,
/// methods, properties, signals, and global functions.
#[derive(Debug, Clone)]
pub struct ClassDb {
    pub source: ClassDbSource,
    classes: HashMap<String, ClassInfo>,
    builtin_classes: HashMap<String, BuiltinClassInfo>,
    utility_functions: HashMap<String, UtilityFunctionInfo>,
    #[allow(dead_code)] // Loaded from Godot API for future use
    global_enums: HashMap<String, EnumInfo>,
    singletons: HashMap<String, String>, // name -> type
}

impl ClassDb {
    /// Create an empty ClassDb with no class information.
    /// Useful for testing or when no bundled database is available.
    pub fn empty() -> Self {
        Self {
            source: ClassDbSource::Bundled {
                version: "empty".to_string(),
            },
            classes: HashMap::new(),
            builtin_classes: HashMap::new(),
            utility_functions: HashMap::new(),
            global_enums: HashMap::new(),
            singletons: HashMap::new(),
        }
    }

    /// List all bundled ClassDB versions available.
    #[allow(dead_code)] // Public API for CLI or library users
    pub fn bundled_versions() -> Vec<&'static str> {
        VERSIONS.iter().map(|(v, _)| *v).collect()
    }

    /// Get the latest (highest) bundled version string.
    #[allow(dead_code)] // Public API for CLI or library users
    pub fn latest_bundled_version() -> &'static str {
        VERSIONS.last().map(|(v, _)| *v).unwrap_or("unknown")
    }

    /// Load a bundled ClassDB by version.
    ///
    /// The version can be:
    /// - An exact match like "4.5.1"
    /// - A major.minor prefix like "4.5" (resolves to the highest patch)
    /// - None to use the latest bundled version
    pub fn from_bundled(version: Option<&str>) -> Result<Self, GdEyeError> {
        let data = match version {
            None => VERSIONS.last().ok_or(GdEyeError::NoBundledVersions)?,
            Some(requested) => resolve_version(requested)?,
        };

        let mut decoder = DeflateDecoder::new(data.1);
        let mut json_bytes = Vec::new();
        decoder
            .read_to_end(&mut json_bytes)
            .map_err(|e| GdEyeError::DecompressError {
                version: data.0.to_string(),
                source: e,
            })?;
        let raw: RawApi = serde_json::from_slice(&json_bytes)?;
        let mut db = Self::from_raw(raw);
        db.source = ClassDbSource::Bundled {
            version: data.0.to_string(),
        };
        Ok(db)
    }

    /// Load from the full (unstripped) extension_api.json format.
    /// This handles the fact that runtime dumps have "return_value": {"type": ...}
    /// instead of the pre-stripped "return_type": "..." format.
    pub fn from_extension_api(json_bytes: &[u8], godot_path: &str) -> Result<Self, GdEyeError> {
        let api: serde_json::Value = serde_json::from_slice(json_bytes)?;

        // Extract version from header
        let version = api
            .get("header")
            .map(|h| {
                let major = h.get("version_major").and_then(|v| v.as_u64()).unwrap_or(4);
                let minor = h.get("version_minor").and_then(|v| v.as_u64()).unwrap_or(0);
                let patch = h.get("version_patch").and_then(|v| v.as_u64()).unwrap_or(0);
                format!("{}.{}.{}", major, minor, patch)
            })
            .unwrap_or_else(|| "unknown".to_string());

        let mut classes = HashMap::new();
        if let Some(cls_array) = api.get("classes").and_then(|v| v.as_array()) {
            for cls in cls_array {
                let info = parse_class_from_value(cls);
                classes.insert(info.name.clone(), info);
            }
        }

        let mut builtin_classes = HashMap::new();
        if let Some(bc_array) = api.get("builtin_classes").and_then(|v| v.as_array()) {
            for bc in bc_array {
                let info = parse_builtin_class_from_value(bc);
                builtin_classes.insert(info.name.clone(), info);
            }
        }

        let mut utility_functions = HashMap::new();
        if let Some(uf_array) = api.get("utility_functions").and_then(|v| v.as_array()) {
            for uf in uf_array {
                let info = parse_utility_function_from_value(uf);
                utility_functions.insert(info.name.clone(), info);
            }
        }

        let mut global_enums = HashMap::new();
        if let Some(ge_array) = api.get("global_enums").and_then(|v| v.as_array()) {
            for ge in ge_array {
                let info = parse_enum_from_value(ge);
                global_enums.insert(info.name.clone(), info);
            }
        }

        let mut singletons = HashMap::new();
        if let Some(s_array) = api.get("singletons").and_then(|v| v.as_array()) {
            for s in s_array {
                let name = s
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let ty = s
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                singletons.insert(name, ty);
            }
        }

        Ok(ClassDb {
            source: ClassDbSource::Runtime {
                version,
                path: godot_path.to_string(),
            },
            classes,
            builtin_classes,
            utility_functions,
            global_enums,
            singletons,
        })
    }

    fn from_raw(raw: RawApi) -> Self {
        let mut classes = HashMap::with_capacity(raw.classes.len());
        for cls in raw.classes {
            let info = ClassInfo {
                name: cls.name.clone(),
                parent: cls.inherits,
                methods: cls
                    .methods
                    .into_iter()
                    .map(|m| MethodInfo {
                        name: m.name,
                        is_static: m.is_static,
                        is_virtual: m.is_virtual,
                        return_type: m.return_type,
                        arguments: m
                            .arguments
                            .into_iter()
                            .map(|a| ArgumentInfo {
                                name: a.name,
                                arg_type: a.arg_type,
                            })
                            .collect(),
                    })
                    .collect(),
                properties: cls
                    .properties
                    .into_iter()
                    .map(|p| PropertyInfo {
                        name: p.name,
                        prop_type: p.prop_type,
                    })
                    .collect(),
                signals: cls
                    .signals
                    .into_iter()
                    .map(|s| SignalInfo {
                        name: s.name,
                        arguments: s
                            .arguments
                            .into_iter()
                            .map(|a| ArgumentInfo {
                                name: a.name,
                                arg_type: a.arg_type,
                            })
                            .collect(),
                    })
                    .collect(),
                constants: cls
                    .constants
                    .into_iter()
                    .map(|c| ConstantInfo {
                        name: c.name,
                        value: c.value.as_i64().unwrap_or(0),
                    })
                    .collect(),
                enums: cls
                    .enums
                    .into_iter()
                    .map(|e| EnumInfo {
                        name: e.name,
                        values: e
                            .values
                            .into_iter()
                            .map(|v| EnumValueInfo {
                                name: v.name,
                                value: v.value,
                            })
                            .collect(),
                    })
                    .collect(),
            };
            classes.insert(info.name.clone(), info);
        }

        let mut builtin_classes = HashMap::with_capacity(raw.builtin_classes.len());
        for bc in raw.builtin_classes {
            let info = BuiltinClassInfo {
                name: bc.name.clone(),
                methods: bc
                    .methods
                    .into_iter()
                    .map(|m| MethodInfo {
                        name: m.name,
                        is_static: m.is_static,
                        is_virtual: false,
                        return_type: m.return_type,
                        arguments: m
                            .arguments
                            .into_iter()
                            .map(|a| ArgumentInfo {
                                name: a.name,
                                arg_type: a.arg_type,
                            })
                            .collect(),
                    })
                    .collect(),
                operators: bc
                    .operators
                    .into_iter()
                    .map(|o| OperatorInfo {
                        name: o.name,
                        right_type: o.right_type,
                        return_type: o.return_type,
                    })
                    .collect(),
            };
            builtin_classes.insert(info.name.clone(), info);
        }

        let mut utility_functions = HashMap::with_capacity(raw.utility_functions.len());
        for uf in raw.utility_functions {
            let info = UtilityFunctionInfo {
                name: uf.name.clone(),
                is_vararg: uf.is_vararg,
                return_type: uf.return_type,
                arguments: uf
                    .arguments
                    .into_iter()
                    .map(|a| ArgumentInfo {
                        name: a.name,
                        arg_type: a.arg_type,
                    })
                    .collect(),
            };
            utility_functions.insert(info.name.clone(), info);
        }

        let mut global_enums = HashMap::with_capacity(raw.global_enums.len());
        for ge in raw.global_enums {
            let info = EnumInfo {
                name: ge.name.clone(),
                values: ge
                    .values
                    .into_iter()
                    .map(|v| EnumValueInfo {
                        name: v.name,
                        value: v.value,
                    })
                    .collect(),
            };
            global_enums.insert(info.name.clone(), info);
        }

        let mut singletons = HashMap::with_capacity(raw.singletons.len());
        for s in raw.singletons {
            singletons.insert(s.name, s.singleton_type);
        }

        ClassDb {
            source: ClassDbSource::Bundled {
                version: String::new(),
            },
            classes,
            builtin_classes,
            utility_functions,
            global_enums,
            singletons,
        }
    }

    // --- Lookup API ---

    /// Get class info by name. Returns None if the class doesn't exist.
    #[allow(dead_code)] // Public API
    pub fn get_class(&self, name: &str) -> Option<&ClassInfo> {
        self.classes.get(name)
    }

    /// Get builtin class info by name (Array, Dictionary, Vector2, etc.)
    #[allow(dead_code)] // Public API
    pub fn get_builtin_class(&self, name: &str) -> Option<&BuiltinClassInfo> {
        self.builtin_classes.get(name)
    }

    /// Check if a class exists in the database (engine or builtin).
    pub fn class_exists(&self, name: &str) -> bool {
        self.classes.contains_key(name) || self.builtin_classes.contains_key(name)
    }

    /// Check if `child` is a subclass of `parent` (walks the inheritance chain).
    pub fn is_subclass_of(&self, child: &str, parent: &str) -> bool {
        if child == parent {
            return true;
        }
        let mut current = child;
        loop {
            match self.classes.get(current) {
                Some(info) if !info.parent.is_empty() => {
                    if info.parent == parent {
                        return true;
                    }
                    current = &info.parent;
                }
                _ => return false,
            }
        }
    }

    /// Get a method on a class, searching up the inheritance chain.
    pub fn get_method(&self, class: &str, method_name: &str) -> Option<&MethodInfo> {
        let mut current = class;
        loop {
            if let Some(info) = self.classes.get(current) {
                if let Some(m) = info.methods.iter().find(|m| m.name == method_name) {
                    return Some(m);
                }
                if info.parent.is_empty() {
                    return None;
                }
                current = &info.parent;
            } else {
                return None;
            }
        }
    }

    /// Get a method on a builtin type.
    pub fn get_builtin_method(&self, builtin: &str, method_name: &str) -> Option<&MethodInfo> {
        self.builtin_classes
            .get(builtin)
            .and_then(|bc| bc.methods.iter().find(|m| m.name == method_name))
    }

    /// Get the return type of a binary operator on a builtin type.
    /// `left_type` is the type of the left operand (e.g., "Vector3").
    /// `op` is the operator symbol (e.g., "*", "+").
    /// `right_type` is the type of the right operand (e.g., "float").
    pub fn get_operator_return_type(
        &self,
        left_type: &str,
        op: &str,
        right_type: &str,
    ) -> Option<String> {
        let bc = self.builtin_classes.get(left_type)?;
        // Find an operator matching the name and right type
        bc.operators
            .iter()
            .find(|o| o.name == op && o.right_type == right_type)
            .map(|o| o.return_type.clone())
    }

    /// Check if a class has a given property, searching up the inheritance chain.
    #[allow(dead_code)] // Public API
    pub fn has_property(&self, class: &str, prop_name: &str) -> bool {
        let mut current = class;
        loop {
            if let Some(info) = self.classes.get(current) {
                if info.properties.iter().any(|p| p.name == prop_name) {
                    return true;
                }
                if info.parent.is_empty() {
                    return false;
                }
                current = &info.parent;
            } else {
                return false;
            }
        }
    }

    /// Get property info, searching up the inheritance chain.
    pub fn get_property(&self, class: &str, prop_name: &str) -> Option<&PropertyInfo> {
        let mut current = class;
        loop {
            if let Some(info) = self.classes.get(current) {
                if let Some(p) = info.properties.iter().find(|p| p.name == prop_name) {
                    return Some(p);
                }
                if info.parent.is_empty() {
                    return None;
                }
                current = &info.parent;
            } else {
                return None;
            }
        }
    }

    /// Check if a class has a given signal, searching up the inheritance chain.
    #[allow(dead_code)] // Public API
    pub fn has_signal(&self, class: &str, signal_name: &str) -> bool {
        self.get_signal(class, signal_name).is_some()
    }

    /// Get signal info by name, searching up the inheritance chain.
    pub fn get_signal(&self, class: &str, signal_name: &str) -> Option<&SignalInfo> {
        let mut current = class;
        loop {
            if let Some(info) = self.classes.get(current) {
                if let Some(sig) = info.signals.iter().find(|s| s.name == signal_name) {
                    return Some(sig);
                }
                if info.parent.is_empty() {
                    return None;
                }
                current = &info.parent;
            } else {
                return None;
            }
        }
    }

    /// Get a utility function by name (print, range, etc.)
    pub fn get_utility_function(&self, name: &str) -> Option<&UtilityFunctionInfo> {
        self.utility_functions.get(name)
    }

    /// Check if a name is a known utility function.
    #[allow(dead_code)] // Public API
    pub fn is_utility_function(&self, name: &str) -> bool {
        self.utility_functions.contains_key(name)
    }

    /// Get a global enum by name.
    #[allow(dead_code)] // Public API
    pub fn get_global_enum(&self, name: &str) -> Option<&EnumInfo> {
        self.global_enums.get(name)
    }

    /// Get the type of a singleton by name.
    pub fn get_singleton_type(&self, name: &str) -> Option<&str> {
        self.singletons.get(name).map(|s| s.as_str())
    }

    /// Check if a name is a known singleton.
    #[allow(dead_code)] // Public API
    pub fn is_singleton(&self, name: &str) -> bool {
        self.singletons.contains_key(name)
    }

    /// Get the full inheritance chain for a class (from child to root).
    #[allow(dead_code)] // Public API
    pub fn inheritance_chain(&self, class: &str) -> Vec<&str> {
        let mut chain = Vec::new();
        let mut current = match self.classes.get(class) {
            Some(info) => {
                chain.push(info.name.as_str());
                info
            }
            None => return chain,
        };
        while !current.parent.is_empty() {
            match self.classes.get(&current.parent) {
                Some(parent_info) => {
                    chain.push(parent_info.name.as_str());
                    current = parent_info;
                }
                None => {
                    // Parent not in DB, add the name anyway
                    chain.push(&current.parent);
                    break;
                }
            }
        }
        chain
    }

    /// Find the most specific common ancestor of two classes.
    ///
    /// Returns `None` if neither class exists in the database or they share no
    /// common ancestor.
    pub fn common_ancestor<'a>(&'a self, class_a: &'a str, class_b: &str) -> Option<&'a str> {
        if class_a == class_b {
            // If both are the same class and it exists, return it
            if self.classes.contains_key(class_a) {
                return Some(class_a);
            }
            return None;
        }

        let chain_a = self.inheritance_chain(class_a);
        if chain_a.is_empty() {
            return None;
        }

        let chain_b: std::collections::HashSet<_> =
            self.inheritance_chain(class_b).into_iter().collect();
        if chain_b.is_empty() {
            return None;
        }

        // Find first ancestor of A that's also an ancestor of B
        chain_a
            .into_iter()
            .find(|&ancestor| chain_b.contains(ancestor))
    }

    /// Find common ancestor of multiple classes.
    ///
    /// Returns `None` if the list is empty, any class doesn't exist, or there's
    /// no common ancestor.
    pub fn common_ancestor_of_all<'a>(&'a self, classes: &[&'a str]) -> Option<&'a str> {
        if classes.is_empty() {
            return None;
        }
        if classes.len() == 1 {
            // Single class: return it if it exists
            if self.classes.contains_key(classes[0]) {
                return Some(classes[0]);
            }
            return None;
        }

        let mut result = classes[0];
        for &class in &classes[1..] {
            result = self.common_ancestor(result, class)?;
        }
        Some(result)
    }

    /// Get the number of classes loaded.
    #[allow(dead_code)] // Public API
    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    /// Get the number of builtin classes loaded.
    #[allow(dead_code)] // Public API
    pub fn builtin_class_count(&self) -> usize {
        self.builtin_classes.len()
    }

    /// Iterate over all class names in the database.
    pub fn class_names(&self) -> impl Iterator<Item = &str> {
        self.classes.keys().map(|s| s.as_str())
    }

    /// Iterate over all builtin class names in the database.
    pub fn builtin_class_names(&self) -> impl Iterator<Item = &str> {
        self.builtin_classes.keys().map(|s| s.as_str())
    }
}

/// Resolve a version string to a bundled (version, data) entry.
///
/// Supports:
/// - Exact match: "4.5.1"
/// - Prefix match: "4.5" -> highest "4.5.x" available
fn resolve_version(requested: &str) -> Result<&'static (&'static str, &'static [u8]), GdEyeError> {
    // Try exact match first
    if let Some(entry) = VERSIONS.iter().find(|(v, _)| *v == requested) {
        return Ok(entry);
    }

    // Try prefix match (e.g., "4.5" matches "4.5.1", "4.5.0", etc.)
    let prefix = format!("{}.", requested);
    let matches: Vec<_> = VERSIONS
        .iter()
        .filter(|(v, _)| v.starts_with(&prefix) || *v == requested)
        .collect();

    if let Some(entry) = matches.last() {
        return Ok(entry);
    }

    let available: Vec<&str> = VERSIONS.iter().map(|(v, _)| *v).collect();
    Err(GdEyeError::UnknownVersion {
        version: requested.to_string(),
        available: available.join(", "),
    })
}

// --- Helpers for parsing full extension_api.json at runtime ---

fn parse_class_from_value(cls: &serde_json::Value) -> ClassInfo {
    let name = cls
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let parent = cls
        .get("inherits")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let methods = cls
        .get("methods")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(parse_method_from_value).collect())
        .unwrap_or_default();

    let properties = cls
        .get("properties")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|p| PropertyInfo {
                    name: p
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    prop_type: p
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    let signals = cls
        .get("signals")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|s| SignalInfo {
                    name: s
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    arguments: s
                        .get("arguments")
                        .and_then(|v| v.as_array())
                        .map(|args| args.iter().map(parse_arg_from_value).collect())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();

    let constants = cls
        .get("constants")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|c| ConstantInfo {
                    name: c
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    value: c.get("value").and_then(|v| v.as_i64()).unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();

    let enums = cls
        .get("enums")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(parse_enum_from_value).collect())
        .unwrap_or_default();

    ClassInfo {
        name,
        parent,
        methods,
        properties,
        signals,
        constants,
        enums,
    }
}

fn parse_method_from_value(m: &serde_json::Value) -> MethodInfo {
    let name = m
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let is_static = m
        .get("is_static")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let is_virtual = m
        .get("is_virtual")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Handle both stripped format ("return_type": "...") and full format ("return_value": {"type": "..."})
    let return_type = m
        .get("return_type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            m.get("return_value")
                .and_then(|rv| rv.get("type"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let arguments = m
        .get("arguments")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(parse_arg_from_value).collect())
        .unwrap_or_default();

    MethodInfo {
        name,
        is_static,
        is_virtual,
        return_type,
        arguments,
    }
}

fn parse_builtin_class_from_value(bc: &serde_json::Value) -> BuiltinClassInfo {
    let name = bc
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let methods = bc
        .get("methods")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|m| {
                    let mname = m
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let is_static = m
                        .get("is_static")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let return_type = m
                        .get("return_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let arguments = m
                        .get("arguments")
                        .and_then(|v| v.as_array())
                        .map(|args| args.iter().map(parse_arg_from_value).collect())
                        .unwrap_or_default();
                    MethodInfo {
                        name: mname,
                        is_static,
                        is_virtual: false,
                        return_type,
                        arguments,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let operators = bc
        .get("operators")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|o| OperatorInfo {
                    name: o
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    right_type: o
                        .get("right_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    return_type: o
                        .get("return_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    BuiltinClassInfo {
        name,
        methods,
        operators,
    }
}

fn parse_utility_function_from_value(uf: &serde_json::Value) -> UtilityFunctionInfo {
    let name = uf
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let is_vararg = uf
        .get("is_vararg")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let return_type = uf
        .get("return_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let arguments = uf
        .get("arguments")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(parse_arg_from_value).collect())
        .unwrap_or_default();
    UtilityFunctionInfo {
        name,
        is_vararg,
        return_type,
        arguments,
    }
}

fn parse_enum_from_value(e: &serde_json::Value) -> EnumInfo {
    let name = e
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let values = e
        .get("values")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| EnumValueInfo {
                    name: v
                        .get("name")
                        .and_then(|val| val.as_str())
                        .unwrap_or("")
                        .to_string(),
                    value: v.get("value").and_then(|val| val.as_i64()).unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default();
    EnumInfo { name, values }
}

fn parse_arg_from_value(arg: &serde_json::Value) -> ArgumentInfo {
    ArgumentInfo {
        name: arg
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        arg_type: arg
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_bundled_classdb() {
        let db = ClassDb::from_bundled(None).expect("Failed to load bundled classdb");
        assert!(
            db.class_count() > 900,
            "Expected 900+ classes, got {}",
            db.class_count()
        );
        assert!(
            db.builtin_class_count() > 30,
            "Expected 30+ builtins, got {}",
            db.builtin_class_count()
        );
    }

    #[test]
    fn class_exists_node() {
        let db = ClassDb::from_bundled(None).unwrap();
        assert!(db.class_exists("Node"));
        assert!(db.class_exists("Node2D"));
        assert!(db.class_exists("Control"));
        assert!(!db.class_exists("FakeClass"));
    }

    #[test]
    fn inheritance_chain() {
        let db = ClassDb::from_bundled(None).unwrap();
        let chain = db.inheritance_chain("Node2D");
        assert_eq!(chain, vec!["Node2D", "CanvasItem", "Node", "Object"]);
    }

    #[test]
    fn is_subclass_of() {
        let db = ClassDb::from_bundled(None).unwrap();
        assert!(db.is_subclass_of("Node2D", "Node"));
        assert!(db.is_subclass_of("Node2D", "Object"));
        assert!(db.is_subclass_of("Node", "Node"));
        assert!(!db.is_subclass_of("Node", "Node2D"));
        assert!(!db.is_subclass_of("Control", "Node2D"));
    }

    #[test]
    fn get_method_inherited() {
        let db = ClassDb::from_bundled(None).unwrap();
        // Node2D should have add_child from Node
        let m = db.get_method("Node2D", "add_child");
        assert!(m.is_some(), "Node2D should inherit add_child from Node");
        // Node2D's own method
        let m2 = db.get_method("Node2D", "get_position");
        assert!(m2.is_some(), "Node2D should have get_position");
    }

    #[test]
    fn builtin_class_methods() {
        let db = ClassDb::from_bundled(None).unwrap();
        let m = db.get_builtin_method("Array", "size");
        assert!(m.is_some());
        assert_eq!(m.unwrap().return_type, "int");
    }

    #[test]
    fn utility_functions() {
        let db = ClassDb::from_bundled(None).unwrap();
        assert!(db.is_utility_function("print"));
        assert!(db.is_utility_function("sin"));
        assert!(!db.is_utility_function("fake_function"));

        let print_fn = db.get_utility_function("print").unwrap();
        assert!(print_fn.is_vararg);

        let sin_fn = db.get_utility_function("sin").unwrap();
        assert!(!sin_fn.is_vararg);
        assert_eq!(sin_fn.return_type, "float");
    }

    #[test]
    fn singletons() {
        let db = ClassDb::from_bundled(None).unwrap();
        assert!(db.is_singleton("Engine"));
        assert_eq!(db.get_singleton_type("Engine"), Some("Engine"));
    }

    #[test]
    fn has_property() {
        let db = ClassDb::from_bundled(None).unwrap();
        // Node has "name" property
        assert!(db.has_property("Node", "name"));
        // Node2D inherits Node's properties
        assert!(db.has_property("Node2D", "name"));
        assert!(!db.has_property("Node", "fake_property"));
    }

    #[test]
    fn has_signal() {
        let db = ClassDb::from_bundled(None).unwrap();
        // Node has "ready" signal
        assert!(db.has_signal("Node", "ready"));
        // Node2D inherits Node's signals
        assert!(db.has_signal("Node2D", "ready"));
    }

    #[test]
    fn global_enums() {
        let db = ClassDb::from_bundled(None).unwrap();
        let side = db.get_global_enum("Side");
        assert!(side.is_some());
    }

    #[test]
    fn common_ancestor_same_class() {
        let db = ClassDb::from_bundled(None).unwrap();
        assert_eq!(db.common_ancestor("Node2D", "Node2D"), Some("Node2D"));
    }

    #[test]
    fn common_ancestor_direct_parent() {
        let db = ClassDb::from_bundled(None).unwrap();
        // Node2D inherits from CanvasItem
        assert_eq!(
            db.common_ancestor("Node2D", "CanvasItem"),
            Some("CanvasItem")
        );
    }

    #[test]
    fn common_ancestor_siblings() {
        let db = ClassDb::from_bundled(None).unwrap();
        // Button and Label both inherit from Control
        assert_eq!(db.common_ancestor("Button", "Label"), Some("Control"));
        // Node2D and Node3D both inherit from Node
        assert_eq!(db.common_ancestor("Node2D", "Node3D"), Some("Node"));
    }

    #[test]
    fn common_ancestor_different_branches() {
        let db = ClassDb::from_bundled(None).unwrap();
        // Sprite2D (Node2D branch) and Label (Control branch) share CanvasItem
        assert_eq!(db.common_ancestor("Sprite2D", "Label"), Some("CanvasItem"));
    }

    #[test]
    fn common_ancestor_unknown_class() {
        let db = ClassDb::from_bundled(None).unwrap();
        assert_eq!(db.common_ancestor("FakeClass", "Node2D"), None);
        assert_eq!(db.common_ancestor("Node2D", "FakeClass"), None);
    }

    #[test]
    fn common_ancestor_of_all_single() {
        let db = ClassDb::from_bundled(None).unwrap();
        assert_eq!(db.common_ancestor_of_all(&["Node2D"]), Some("Node2D"));
    }

    #[test]
    fn common_ancestor_of_all_multiple() {
        let db = ClassDb::from_bundled(None).unwrap();
        // Button, Label, and LineEdit all inherit from Control
        assert_eq!(
            db.common_ancestor_of_all(&["Button", "Label", "LineEdit"]),
            Some("Control")
        );
    }

    #[test]
    fn common_ancestor_of_all_empty() {
        let db = ClassDb::from_bundled(None).unwrap();
        assert_eq!(db.common_ancestor_of_all(&[]), None);
    }

    #[test]
    fn operator_lookup_int_plus_float() {
        let db = ClassDb::from_bundled(None).unwrap();
        // int + float should return float
        let result = db.get_operator_return_type("int", "+", "float");
        assert_eq!(result, Some("float".to_string()));
    }

    #[test]
    fn operator_lookup_vector_times_scalar() {
        let db = ClassDb::from_bundled(None).unwrap();
        // Vector3 * float should return Vector3
        let result = db.get_operator_return_type("Vector3", "*", "float");
        assert_eq!(result, Some("Vector3".to_string()));
    }
}
