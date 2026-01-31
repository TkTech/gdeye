# gdeye

<img src="misc/gdeye_logo.png" alt="gdeye logo" width="200">

A fast static analysis, formatting, and LSP for GDScript (Godot 4.x). Built on
[tree-sitter-gdscript](https://github.com/PrestonKnopp/tree-sitter-gdscript)
for fast, accurate parsing. Opinionated by default, but customizable.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE.md)
[![Crates.io](https://img.shields.io/crates/v/gdeye.svg)](https://crates.io/crates/gdeye)
[![CI](https://img.shields.io/github/actions/workflow/status/TkTech/gdeye/ci.yml?branch=main&label=CI)](https://github.com/TkTech/gdeye/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/TkTech/gdeye/branch/main/graph/badge.svg)](https://codecov.io/gh/TkTech/gdeye)

## Usage

```sh
# Analyze an entire Godot project
gdeye check /path/to/godot/project

# Apply available auto-fixes (COMMIT FIRST!)
gdeye check --fix .

# Apply even dangerous fixes (such as removing dead functions)
gdeye check --fix --unsafe .

# Format GDScript files (check mode - shows diff)
gdeye fmt .

# Format in place
gdeye fmt --write .

# List the available linter rules
gdeye rules
```

## Configuration

gdeye looks for a `gdeye.toml` file in the Godot project root (next to
`project.godot`). All options can also be passed on the command line.

```toml
# Glob patterns - only analyze matching files (default: all .gd files)
include = ["scripts/**/*.gd"]

# Glob patterns - skip matching files (merged with CLI --exclude)
exclude = ["addons/**", "test/**"]

# Rule severity: "off", "info", "warning", or "error"
[rules]
"perf/process-allocation" = "error"
# Rule too annoying? Nuke it.
"correctness/unused-parameter" = "off"

# Formatter settings
[formatter]
print_width = 100       # Maximum line width (default: 100)
indent_style = "tabs"   # "tabs" or "spaces" (default: "tabs")
indent_size = 4         # Spaces per indent when using spaces (default: 4)
quote_style = "preserve" # "double", "single", or "preserve" (default: "preserve")
trailing_comma = "multiline" # "all", "multiline", or "none" (default: "multiline")
max_blank_lines = 2     # Max consecutive blank lines (default: 2)

# Rule-specific options
[rules."style/function-too-long"]
max_length = 100  # Maximum lines per function (default: 80)

[rules."style/excessive-nesting"]
max_depth = 6  # Maximum nesting depth (default: 5)
```

Use `gdeye rules <name>` to see available options for any rule.

### CLI equivalents

```sh
# Exclude files
gdeye check --exclude "addons/**" /path/to/project

# Disable rules
gdeye check --disable correctness/unused-variable --disable correctness/unused-signal .

# Restrict to specific files
gdeye check --include "scripts/networking/**/*.gd" .
```

Use `gdeye check --help` for all available options.

### Inline suppression

Suppress warnings on specific lines with comments:

```gdscript
var x = 5  # gdeye:ignore
var y = 5  # gdeye:ignore correctness/unused-variable

# gdeye:ignore-next-line
var z = 5

# gdeye:ignore-next-line correctness/unused-variable
var w = 5
```

## Building

```sh
cargo build --release
```

To build without LSP or MCP support (smaller binary):

```sh
cargo build --release --no-default-features
```

## Editor Integration

gdeye provides an (experimental) Language Server Protocol (LSP) implementation
for real-time diagnostics, hover information, go-to-definition, and more. This
is still under active development and might have some rough edges, especially
around completions.

### Kate

Go to *Settings → Configure Kate → LSP Client → User Server Settings* and add:

```json
{
  "servers": {
    "godot": {
      "command": ["gdeye", "lsp"],
      "root": "%{Project:NativePath}",
      "highlightingModeRegex": "^Godot$"
    }
  }
}
```

## MCP Server

gdeye includes an MCP (Model Context Protocol) server for AI assistants. Add
to your `.mcp.json`:

```json
{
  "mcpServers": {
    "gdeye": {
      "command": "gdeye",
      "args": ["mcp"]
    }
  }
}
```

## Alternatives

- [godot-gdscript-toolkit](https://github.com/Scony/godot-gdscript-toolkit)
- [GDScript-formatter](https://github.com/GDQuest/GDScript-formatter)
- [GDShrapt](https://github.com/elamaunt/GDShrapt)
- [godot-gdscript-linter](https://github.com/graydwarf/godot-gdscript-linter)