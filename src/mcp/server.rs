//! MCP server implementation for gdeye.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::ServerCapabilities;
use rmcp::schemars;
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::classdb::ClassDb;
use crate::config::Config;
use crate::fmt::{self, FmtConfig};
use crate::project::ProjectInfo;
use crate::rules::{self, Severity};

use super::types::*;

/// Parameters for the format_snippet tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FormatSnippetToolParams {
    /// The GDScript source code to format.
    pub source: String,
    /// Maximum line width (default: 100).
    #[serde(default)]
    pub print_width: Option<usize>,
}

/// Parameters for the lint_snippet tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LintSnippetToolParams {
    /// The GDScript source code to lint.
    pub source: String,
    /// Optional filename for diagnostics (default: "snippet.gd").
    #[serde(default)]
    pub filename: Option<String>,
    /// Rule IDs to disable.
    #[serde(default)]
    pub disable: Vec<String>,
}

/// Parameters for the get_rule_info tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetRuleInfoToolParams {
    /// The rule ID to get information about (e.g., "correctness/unused-variable").
    pub rule_id: String,
}

/// Parameters for the lookup_godot_class tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct LookupGodotClassParams {
    /// The class name to look up (e.g., "Node2D", "Array", "Vector3").
    pub class_name: String,
}

/// Parameters for the get_symbols tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolsParams {
    /// The GDScript source code to extract symbols from.
    pub source: String,
}

/// Parameters for the apply_fix tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApplyFixParams {
    /// The GDScript source code to fix.
    pub source: String,
    /// Whether to include unsafe fixes (e.g., removing dead code).
    #[serde(default)]
    pub include_unsafe: bool,
}

/// Parameters for the parse_scene tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParseSceneParams {
    /// The .tscn file content to parse.
    pub content: String,
}

/// Parameters for the list_autoloads tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListAutoloadsParams {
    /// The project.godot file content.
    pub content: String,
}

/// Parameters for the get_call_graph tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetCallGraphParams {
    /// The GDScript source code to analyze.
    pub source: String,
}

/// Parameters for the check_type tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckTypeParams {
    /// The GDScript source code containing the expression.
    pub source: String,
    /// The variable or expression name to check the type of.
    pub name: String,
}

/// The gdeye MCP server, providing linting and formatting tools.
#[derive(Clone)]
pub struct GdeyeMcpServer {
    tool_router: ToolRouter<Self>,
    class_db: Arc<ClassDb>,
}

impl GdeyeMcpServer {
    /// Create a new MCP server instance.
    pub fn new() -> Self {
        let class_db = ClassDb::from_bundled(None).unwrap_or_else(|_| ClassDb::empty());
        Self {
            tool_router: Self::tool_router(),
            class_db: Arc::new(class_db),
        }
    }
}

impl Default for GdeyeMcpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl GdeyeMcpServer {
    /// Format GDScript source code.
    #[tool(description = "Format GDScript source code according to style guidelines")]
    async fn format_snippet(
        &self,
        Parameters(params): Parameters<FormatSnippetToolParams>,
    ) -> String {
        let mut config = FmtConfig::default();
        if let Some(width) = params.print_width {
            config.print_width = width;
        }

        match fmt::format_source(&params.source, &config) {
            Ok(result) => {
                let response = FormatResponse {
                    formatted: result.output,
                    unchanged: result.unchanged,
                };
                serde_json::to_string_pretty(&response).unwrap_or_default()
            }
            Err(e) => {
                format!("{{\"error\": \"{}\"}}", e.replace('"', "\\\""))
            }
        }
    }

    /// Lint GDScript code and return diagnostics.
    #[tool(description = "Lint GDScript code and return diagnostics for potential issues")]
    async fn lint_snippet(&self, Parameters(params): Parameters<LintSnippetToolParams>) -> String {
        let filename = params.filename.unwrap_or_else(|| "snippet.gd".to_string());
        let path = Path::new(&filename);

        // Build config with disabled rules
        let mut config = Config::default();
        for rule_id in &params.disable {
            config.disable.push(rule_id.clone());
        }

        let scenes = HashMap::new();
        let project_info = ProjectInfo::default();

        match crate::analyze_source_impl(
            path,
            &params.source,
            &self.class_db,
            &config,
            &scenes,
            &project_info,
        ) {
            Ok(analysis) => {
                let diagnostics: Vec<DiagnosticInfo> = analysis
                    .diagnostics
                    .iter()
                    .map(|d| DiagnosticInfo {
                        rule: d.rule.to_string(),
                        severity: match d.severity {
                            Severity::Error => "error".to_string(),
                            Severity::Warning => "warning".to_string(),
                            Severity::Info => "info".to_string(),
                        },
                        message: d.message.clone(),
                        line: d.line,
                        col: d.col,
                        end_line: d.end_line,
                        end_col: d.end_col,
                        note: d.note.clone(),
                    })
                    .collect();

                let response = LintResponse { diagnostics };
                serde_json::to_string_pretty(&response).unwrap_or_default()
            }
            Err(e) => {
                format!("{{\"error\": \"{}\"}}", e.to_string().replace('"', "\\\""))
            }
        }
    }

    /// List all available lint rules.
    #[tool(description = "List all available lint rules with their descriptions and severities")]
    async fn list_rules(&self) -> String {
        let all_rules = rules::all_rules();
        let rules_info: Vec<RuleSummary> = all_rules
            .iter()
            .map(|r| RuleSummary {
                id: r.id().to_string(),
                description: r.description().to_string(),
                default_severity: format!("{}", r.default_severity()),
                category: r.category().to_string(),
            })
            .collect();

        let response = ListRulesResponse { rules: rules_info };
        serde_json::to_string_pretty(&response).unwrap_or_default()
    }

    /// Get detailed information about a specific lint rule.
    #[tool(description = "Get detailed information about a specific lint rule by its ID")]
    async fn get_rule_info(&self, Parameters(params): Parameters<GetRuleInfoToolParams>) -> String {
        let all_rules = rules::all_rules();
        let rule = all_rules.iter().find(|r| r.id() == params.rule_id);

        match rule {
            Some(r) => {
                let options: Vec<RuleOptionInfo> = r
                    .options()
                    .iter()
                    .map(|opt| RuleOptionInfo {
                        name: opt.name.to_string(),
                        description: opt.description.to_string(),
                        default: opt.default.to_string(),
                        value_type: format!("{}", opt.value_type),
                    })
                    .collect();

                let response = RuleInfoResponse {
                    id: r.id().to_string(),
                    description: r.description().to_string(),
                    default_severity: format!("{}", r.default_severity()),
                    category: r.category().to_string(),
                    options,
                };
                serde_json::to_string_pretty(&response).unwrap_or_default()
            }
            None => {
                format!("{{\"error\": \"Unknown rule: {}\"}}", params.rule_id)
            }
        }
    }

    /// Look up a Godot built-in class, returning its methods, properties, and signals.
    #[tool(
        description = "Look up a Godot built-in class (e.g., Node2D, Array, Vector3) and return its methods, properties, and signals"
    )]
    async fn lookup_godot_class(
        &self,
        Parameters(params): Parameters<LookupGodotClassParams>,
    ) -> String {
        // Try engine class first
        if let Some(class_info) = self.class_db.get_class(&params.class_name) {
            let response = GodotClassResponse {
                name: class_info.name.clone(),
                parent: if class_info.parent.is_empty() {
                    None
                } else {
                    Some(class_info.parent.clone())
                },
                is_builtin: false,
                methods: class_info
                    .methods
                    .iter()
                    .map(|m| MethodSummary {
                        name: m.name.clone(),
                        return_type: if m.return_type.is_empty() {
                            None
                        } else {
                            Some(m.return_type.clone())
                        },
                        arguments: m
                            .arguments
                            .iter()
                            .map(|a| ArgumentSummary {
                                name: a.name.clone(),
                                arg_type: a.arg_type.clone(),
                            })
                            .collect(),
                        is_static: m.is_static,
                        is_virtual: m.is_virtual,
                    })
                    .collect(),
                properties: class_info
                    .properties
                    .iter()
                    .map(|p| PropertySummary {
                        name: p.name.clone(),
                        prop_type: p.prop_type.clone(),
                    })
                    .collect(),
                signals: class_info
                    .signals
                    .iter()
                    .map(|s| SignalSummary {
                        name: s.name.clone(),
                        arguments: s
                            .arguments
                            .iter()
                            .map(|a| ArgumentSummary {
                                name: a.name.clone(),
                                arg_type: a.arg_type.clone(),
                            })
                            .collect(),
                    })
                    .collect(),
                constants: class_info
                    .constants
                    .iter()
                    .map(|c| ConstantSummary {
                        name: c.name.clone(),
                        value: c.value,
                    })
                    .collect(),
            };
            return serde_json::to_string_pretty(&response).unwrap_or_default();
        }

        // Try builtin class (Vector2, Array, etc.)
        if let Some(builtin_info) = self.class_db.get_builtin_class(&params.class_name) {
            let response = GodotClassResponse {
                name: builtin_info.name.clone(),
                parent: None,
                is_builtin: true,
                methods: builtin_info
                    .methods
                    .iter()
                    .map(|m| MethodSummary {
                        name: m.name.clone(),
                        return_type: if m.return_type.is_empty() {
                            None
                        } else {
                            Some(m.return_type.clone())
                        },
                        arguments: m
                            .arguments
                            .iter()
                            .map(|a| ArgumentSummary {
                                name: a.name.clone(),
                                arg_type: a.arg_type.clone(),
                            })
                            .collect(),
                        is_static: m.is_static,
                        is_virtual: m.is_virtual,
                    })
                    .collect(),
                properties: vec![],
                signals: vec![],
                constants: vec![],
            };
            return serde_json::to_string_pretty(&response).unwrap_or_default();
        }

        format!("{{\"error\": \"Unknown class: {}\"}}", params.class_name)
    }

    /// Extract symbols (functions, variables, signals, etc.) from GDScript source.
    #[tool(
        description = "Extract symbols (functions, variables, signals, classes, enums) from GDScript source code"
    )]
    async fn get_symbols(&self, Parameters(params): Parameters<GetSymbolsParams>) -> String {
        let path = Path::new("snippet.gd");
        match crate::parser::parse_source(&params.source) {
            Ok(parsed) => {
                let symbols = crate::symbols::collect_symbols(path, &parsed);
                let response = SymbolsResponse {
                    class_name: symbols.class_name,
                    extends: symbols.extends,
                    signals: symbols
                        .signals
                        .iter()
                        .map(|s| SymbolSignal {
                            name: s.name.clone(),
                            parameters: s.parameters.clone(),
                            line: s.line,
                        })
                        .collect(),
                    enums: symbols
                        .enums
                        .iter()
                        .map(|e| SymbolEnum {
                            name: e.name.clone(),
                            values: e.values.clone(),
                            line: e.line,
                        })
                        .collect(),
                    constants: symbols
                        .constants
                        .iter()
                        .map(|c| SymbolConst {
                            name: c.name.clone(),
                            type_annotation: c.type_annotation.clone(),
                            line: c.line,
                        })
                        .collect(),
                    variables: symbols
                        .variables
                        .iter()
                        .map(|v| SymbolVar {
                            name: v.name.clone(),
                            type_annotation: v.type_annotation.clone(),
                            inferred_type: v.inferred_type.clone(),
                            is_onready: v.is_onready,
                            is_export: v.is_export,
                            line: v.line,
                        })
                        .collect(),
                    functions: symbols
                        .functions
                        .iter()
                        .map(|f| SymbolFunc {
                            name: f.name.clone(),
                            parameters: f
                                .parameters
                                .iter()
                                .map(|p| SymbolParam {
                                    name: p.name.clone(),
                                    type_annotation: p.type_annotation.clone(),
                                })
                                .collect(),
                            return_type: f.return_type.clone(),
                            line: f.line,
                            end_line: f.end_line,
                        })
                        .collect(),
                };
                serde_json::to_string_pretty(&response).unwrap_or_default()
            }
            Err(e) => format!("{{\"error\": \"{}\"}}", e.replace('"', "\\\"")),
        }
    }

    /// Apply available fixes to GDScript source code.
    #[tool(
        description = "Lint GDScript code and apply available auto-fixes, returning the fixed source"
    )]
    async fn apply_fix(&self, Parameters(params): Parameters<ApplyFixParams>) -> String {
        let path = Path::new("snippet.gd");
        let config = Config::default();
        let scenes = HashMap::new();
        let project_info = ProjectInfo::default();

        match crate::analyze_source_impl(
            path,
            &params.source,
            &self.class_db,
            &config,
            &scenes,
            &project_info,
        ) {
            Ok(analysis) => {
                let fix_result = crate::fix::apply_fixes(
                    &params.source,
                    &analysis.diagnostics,
                    params.include_unsafe,
                );
                let response = ApplyFixResponse {
                    fixed_source: fix_result.source,
                    num_fixed: fix_result.num_fixed,
                    had_fixes: !analysis.diagnostics.iter().all(|d| d.fix.is_none()),
                };
                serde_json::to_string_pretty(&response).unwrap_or_default()
            }
            Err(e) => format!("{{\"error\": \"{}\"}}", e.to_string().replace('"', "\\\"")),
        }
    }

    /// Parse a .tscn scene file and return its structure.
    #[tool(
        description = "Parse a Godot .tscn scene file and return its node tree, resources, and signal connections"
    )]
    async fn parse_scene(&self, Parameters(params): Parameters<ParseSceneParams>) -> String {
        // Parse the scene content directly (simplified inline parsing)
        let mut resources = Vec::new();
        let mut nodes = Vec::new();
        let mut connections = Vec::new();

        let lines: Vec<&str> = params.content.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            if line.starts_with("[ext_resource") {
                let attrs = parse_section_attrs(line);
                resources.push(SceneResource {
                    id: attrs.get("id").cloned().unwrap_or_default(),
                    resource_type: attrs.get("type").cloned().unwrap_or_default(),
                    path: attrs.get("path").cloned().unwrap_or_default(),
                });
            } else if line.starts_with("[node") {
                let attrs = parse_section_attrs(line);
                let name = attrs.get("name").cloned().unwrap_or_default();
                let node_type = attrs.get("type").cloned().unwrap_or_default();
                let parent = attrs.get("parent").cloned().unwrap_or_default();

                // Build node path
                let node_path = if parent.is_empty() || parent == "." {
                    name.clone()
                } else {
                    format!("{}/{}", parent, name)
                };

                // Check for script in following lines
                let mut script = None;
                let mut j = i + 1;
                while j < lines.len() && !lines[j].starts_with('[') {
                    let prop_line = lines[j].trim();
                    if prop_line.starts_with("script") {
                        if let Some(start) = prop_line.find("ExtResource(") {
                            if let Some(end) = prop_line[start..].find(')') {
                                let id_part = &prop_line[start + 12..start + end];
                                let id = id_part.trim_matches('"').trim();
                                // Find the corresponding resource path
                                for res in &resources {
                                    if res.id == id || res.id == format!("\"{}\"", id) {
                                        script = Some(res.path.clone());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    j += 1;
                }

                nodes.push(SceneNodeInfo {
                    name,
                    node_type,
                    path: node_path,
                    script,
                });
            } else if line.starts_with("[connection") {
                let attrs = parse_section_attrs(line);
                connections.push(SceneConnection {
                    signal: attrs.get("signal").cloned().unwrap_or_default(),
                    from_node: attrs.get("from").cloned().unwrap_or_default(),
                    to_node: attrs.get("to").cloned().unwrap_or_default(),
                    method: attrs.get("method").cloned().unwrap_or_default(),
                });
            }

            i += 1;
        }

        let response = SceneResponse {
            resources,
            nodes,
            connections,
        };
        serde_json::to_string_pretty(&response).unwrap_or_default()
    }

    /// List autoloads and input actions from a project.godot file.
    #[tool(
        description = "Parse a project.godot file and return autoload singletons and input actions"
    )]
    async fn list_autoloads(&self, Parameters(params): Parameters<ListAutoloadsParams>) -> String {
        let mut project_name = String::new();
        let mut autoloads = Vec::new();
        let mut input_actions = Vec::new();
        let mut current_section = String::new();

        for line in params.content.lines() {
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
                            project_name = unquote(value);
                        }
                    }
                    "autoload" => {
                        let script_path = unquote(value);
                        let script_path = script_path.trim_start_matches('*');
                        autoloads.push(AutoloadEntry {
                            name: key.to_string(),
                            script_path: script_path.to_string(),
                        });
                    }
                    "input" => {
                        input_actions.push(key.to_string());
                    }
                    _ => {}
                }
            }
        }

        let response = AutoloadsResponse {
            project_name,
            autoloads,
            input_actions,
        };
        serde_json::to_string_pretty(&response).unwrap_or_default()
    }

    /// Get function call information from GDScript source.
    #[tool(
        description = "Analyze GDScript source and return function definitions and call relationships"
    )]
    async fn get_call_graph(&self, Parameters(params): Parameters<GetCallGraphParams>) -> String {
        let path = Path::new("snippet.gd");
        match crate::parser::parse_source(&params.source) {
            Ok(parsed) => {
                let symbols = crate::symbols::collect_symbols(path, &parsed);

                // Extract function info
                let functions: Vec<CallGraphFunction> = symbols
                    .functions
                    .iter()
                    .map(|f| CallGraphFunction {
                        name: f.name.clone(),
                        line: f.line,
                        param_count: f.parameters.len(),
                    })
                    .collect();

                // Extract calls (simplified - just find call expressions)
                let mut calls = Vec::new();
                extract_calls(&parsed, &symbols, &mut calls);

                let response = CallGraphResponse { functions, calls };
                serde_json::to_string_pretty(&response).unwrap_or_default()
            }
            Err(e) => format!("{{\"error\": \"{}\"}}", e.replace('"', "\\\"")),
        }
    }

    /// Check the inferred type of a variable or expression.
    #[tool(description = "Infer the type of a variable in GDScript source code")]
    async fn check_type(&self, Parameters(params): Parameters<CheckTypeParams>) -> String {
        let path = Path::new("snippet.gd");
        match crate::parser::parse_source(&params.source) {
            Ok(parsed) => {
                let mut symbols = crate::symbols::collect_symbols(path, &parsed);
                crate::types::propagate_types(&mut symbols, &parsed, &self.class_db);

                // Look for the variable in class-level vars
                for var in &symbols.variables {
                    if var.name == params.name {
                        let inferred = var
                            .type_annotation
                            .clone()
                            .or_else(|| var.inferred_type.clone())
                            .or_else(|| var.initializer_type.clone());
                        let response = CheckTypeResponse {
                            inferred_type: inferred.clone(),
                            explanation: match (&var.type_annotation, &var.inferred_type) {
                                (Some(t), _) => format!("Explicit type annotation: {}", t),
                                (None, Some(t)) => format!("Inferred from usage: {}", t),
                                (None, None) => "Could not determine type".to_string(),
                            },
                        };
                        return serde_json::to_string_pretty(&response).unwrap_or_default();
                    }
                }

                // Look in function local vars
                for func in &symbols.functions {
                    for var in &func.local_vars {
                        if var.name == params.name {
                            let inferred = var
                                .type_annotation
                                .clone()
                                .or_else(|| var.inferred_type.clone())
                                .or_else(|| var.initializer_type.clone());
                            let response = CheckTypeResponse {
                                inferred_type: inferred.clone(),
                                explanation: match (&var.type_annotation, &var.inferred_type) {
                                    (Some(t), _) => format!("Explicit type annotation: {}", t),
                                    (None, Some(t)) => format!("Inferred from usage: {}", t),
                                    (None, None) => "Could not determine type".to_string(),
                                },
                            };
                            return serde_json::to_string_pretty(&response).unwrap_or_default();
                        }
                    }

                    // Check function parameters
                    for param in &func.parameters {
                        if param.name == params.name {
                            let response = CheckTypeResponse {
                                inferred_type: param.type_annotation.clone(),
                                explanation: match &param.type_annotation {
                                    Some(t) => format!("Parameter type annotation: {}", t),
                                    None => "Parameter has no type annotation".to_string(),
                                },
                            };
                            return serde_json::to_string_pretty(&response).unwrap_or_default();
                        }
                    }
                }

                let response = CheckTypeResponse {
                    inferred_type: None,
                    explanation: format!("Variable '{}' not found in source", params.name),
                };
                serde_json::to_string_pretty(&response).unwrap_or_default()
            }
            Err(e) => format!("{{\"error\": \"{}\"}}", e.replace('"', "\\\"")),
        }
    }
}

/// Parse section attributes from a .tscn line like `[node name="Foo" type="Node2D"]`
fn parse_section_attrs(line: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let inner = line.trim_start_matches('[').trim_end_matches(']').trim();

    // Skip the section keyword
    let after_keyword = if let Some(pos) = inner.find(' ') {
        &inner[pos + 1..]
    } else {
        return attrs;
    };

    // Simple key="value" parsing
    let mut remaining = after_keyword;
    while !remaining.is_empty() {
        remaining = remaining.trim_start();
        if let Some(eq_pos) = remaining.find('=') {
            let key = remaining[..eq_pos].trim();
            let rest = &remaining[eq_pos + 1..];

            if let Some(stripped) = rest.strip_prefix('"') {
                // Quoted value
                if let Some(end_quote) = stripped.find('"') {
                    let value = &stripped[..end_quote];
                    attrs.insert(key.to_string(), value.to_string());
                    remaining = &stripped[end_quote + 1..];
                } else {
                    break;
                }
            } else {
                // Unquoted value (until space or end)
                let end = rest.find(' ').unwrap_or(rest.len());
                let value = &rest[..end];
                attrs.insert(key.to_string(), value.to_string());
                remaining = &rest[end..];
            }
        } else {
            break;
        }
    }

    attrs
}

/// Remove surrounding quotes from a value.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Extract function calls from parsed source.
fn extract_calls(
    parsed: &crate::parser::ParsedFile,
    symbols: &crate::symbols::FileSymbols,
    calls: &mut Vec<CallInfo>,
) {
    // Find which function we're currently in based on byte offset
    fn find_enclosing_function(
        byte_offset: usize,
        symbols: &crate::symbols::FileSymbols,
    ) -> Option<String> {
        for func in &symbols.functions {
            if byte_offset >= func.start_byte && byte_offset <= func.end_byte {
                return Some(func.name.clone());
            }
        }
        None
    }

    // Walk the tree looking for call expressions
    fn walk_for_calls(
        node: tree_sitter::Node,
        parsed: &crate::parser::ParsedFile,
        symbols: &crate::symbols::FileSymbols,
        calls: &mut Vec<CallInfo>,
    ) {
        if node.kind() == "call" {
            // Get the function being called
            if let Some(func_node) = node.child_by_field_name("function") {
                let callee = parsed.node_text(func_node).to_string();
                let line = node.start_position().row + 1;
                let caller = find_enclosing_function(node.start_byte(), symbols)
                    .unwrap_or_else(|| "<module>".to_string());

                // Only track simple function calls, not method calls on objects
                if !callee.contains('.') {
                    calls.push(CallInfo {
                        caller,
                        callee,
                        line,
                    });
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk_for_calls(child, parsed, symbols, calls);
        }
    }

    walk_for_calls(parsed.root_node(), parsed, symbols, calls);
}

#[tool_handler]
impl ServerHandler for GdeyeMcpServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo {
            instructions: Some(
                "gdeye - Static analysis and formatting tools for GDScript (Godot 4.x)".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
