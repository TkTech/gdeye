extends Node

# Should warn: parameter not used in non-callback function
func add(a, b, unused_param):
	return a + b

# Should NOT warn: _ prefix suppresses
func transform(value, _context):
	return value * 2

# Should NOT warn: Godot callback functions
func _process(delta):
	pass

func _input(event):
	pass

# Should NOT warn: all params used
func multiply(x, y):
	return x * y

# Should warn: typed parameter unused
func typed_unused(name: String, value: int, unused: float):
	print(name)
	return value
