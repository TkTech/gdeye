extends Node

var cached_array = [1, 2, 3]

func _process(delta):
	# Should warn: array literal in _process
	var arr = [1, 2, 3]
	print(arr)

	# Should warn: Dictionary allocation in _process
	var dict = {"key": "value"}
	print(dict)

	# Should NOT warn: using a cached member variable
	print(cached_array)

func _physics_process(delta):
	# Should warn: Array() constructor in _physics_process
	var arr = Array()
	print(arr)

func _input(event):
	# Should warn: .new() instantiation in input handler
	var obj = Node2D.new()
	print(obj)

	# Should warn: get_node() in process-like function
	var node = get_node("path/to/node")
	print(node)

func some_function():
	# Should NOT warn: not in a process function
	var arr = [1, 2, 3]
	print(arr)
