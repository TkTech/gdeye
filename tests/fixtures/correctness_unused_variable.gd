extends Node

# Should warn: member variable never used
var unused_member = 10

# Should NOT warn: member variable used in a function
var used_member = 20

# Should NOT warn: exported variables are always "used" by the editor
@export var exported_value: int = 5

# Should NOT warn: prefixed with _
var _intentionally_unused = 0

func example():
	# Should warn: local variable unused
	var unused_local = "hello"

	# Should NOT warn: local variable used
	var used_local = "world"
	print(used_local)

	# Should NOT warn: used in expression
	var speed = 10.0
	position += Vector3.UP * speed

	# Use the member variable
	print(used_member)

func uses_variable_in_condition():
	var flag = true
	if flag:
		print("yes")

func uses_variable_in_return():
	var result = calculate()
	return result

func uses_variable_in_for():
	var items = [1, 2, 3]
	for item in items:
		print(item)

func calculate():
	return 42
