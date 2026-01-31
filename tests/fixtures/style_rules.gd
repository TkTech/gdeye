extends Node

class_name myBadClassName

class inner_bad_class:
	var x: int = 0

class GoodInnerClass:
	var y: int = 0

# Bad: PascalCase function name
func MyFunction() -> void:
	pass

# Bad: camelCase function name
func camelCase() -> void:
	pass

# Good: snake_case function name
func good_function() -> void:
	pass

# Bad: PascalCase variable
var BadVar: int = 0

# Good: snake_case variable
var good_var: int = 0

# Untyped parameter
func untyped_param(x, y) -> int:
	return x + y

# Typed parameters (should not flag)
func typed_param(x: int, y: int) -> int:
	return x + y

# No return type
func no_return():
	pass

# With return type (should not flag)
func with_return() -> void:
	pass
