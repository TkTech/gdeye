# Feature Ideas

## Godot-Specific Deep Analysis

### `@export` Validation
Validate that `@export` annotations are compatible with their types, that `@export_range` bounds are sane (min < max), that `@export_enum` values match usage, and that `@export_node_path` types exist in the class database.

### Animation Track Path Validation
Parse `.tres`/`.tscn` animation resources and validate that animation track node paths still resolve correctly against the scene tree.

### Group Name Typo Detection
Cross-reference `add_to_group("enemies")` / `get_tree().get_nodes_in_group("enemis")` calls across the project and flag probable typos via edit-distance matching.

## Project-Level Intelligence

### Dead Scene / Dead Resource Detection
Walk the entire resource graph from the main scene and flag `.tscn`, `.tres`, `.gd` files that are completely unreachable.

### Signal Connection Graph Export
CLI command (`gdeye signals`) that outputs a DOT/Mermaid graph of all signal connections.

## Style Rules

### Consistent Return
Some code paths return a value, others return nothing. Broader than `missing-return` which only checks typed functions.

### No Lonely If
`if` inside `else` that could be rewritten as `elif`.

### Debug Print
`print()`/`push_warning()`/`push_error()` left in production code.

### Magic Number
Hardcoded numeric literals (other than 0, 1, -1) that should be named constants.

### Commented-Out Code
Heuristic detection of commented-out code blocks.

### Empty Function
Functions with no body at all.

### TODO/FIXME Comments
Track `TODO`/`FIXME`/`HACK` comments as technical debt markers.

### Member Ordering
Enforce class member order: signals, enums, constants, exports, vars, onready vars, functions.

### Trailing Comma
Enforce trailing commas in multi-line arrays, dictionaries, and parameter lists.

## Complexity Metrics

### Cyclomatic Complexity
Numeric complexity score per function with configurable threshold.

### Max Parameters
Too many function parameters (configurable, default: 4-5).

### God Class
Class has too many public methods, signals, or variables.

### Max Boolean Expressions
Too many `and`/`or` operators in a single condition.

### Max Branches
Too many if/elif/match branches in one function.

### Max Class Variables
Too many class-level variable declarations.

### Max Local Variables
Too many local variables in a function.

### Max Public Methods
Too many public methods on a class.

### Max Returns
Too many return statements in a function.

### Max File Lines
File exceeding a configurable line count threshold.

### Max Inner Classes
Too many inner classes in a single file.
