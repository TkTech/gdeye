extends Node

func returns_wrong_literal() -> int:
    return "hello"

func returns_correct_literal() -> int:
    return 42

func returns_wrong_call() -> String:
    return get_viewport()

func returns_compatible_numeric() -> float:
    return 42

func no_return_type():
    return "anything"

# Additional cases for coverage: ternary, parenthesized, string concat
func ternary_return() -> int:
    return 1 if true else 0

func paren_return() -> int:
    return (42)

func concat_return() -> String:
    return "hello" + " world"

# Inferred type with := should not cause false positives
func inferred_type_new() -> Node3D:
    var container := Node3D.new()
    return container

func inferred_type_literal() -> int:
    var x := 42
    return x

# Cast expression should be respected
func cast_return() -> Node3D:
    var scene: PackedScene = load("res://test.tscn")
    return scene.instantiate() as Node3D

# Lambda return statements should not be attributed to enclosing function
func uses_lambda_sort() -> Array:
    var items = [3, 1, 2]
    items.sort_custom(func(a, b): return a > b)
    return items
