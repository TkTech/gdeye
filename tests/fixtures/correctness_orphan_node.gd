extends Node

func test_orphan():
	var node = Node2D.new()
	print("created node")

func test_unassigned():
	Sprite2D.new()

func test_safe():
	var child = Node2D.new()
	add_child(child)

func test_returned():
	var node = Control.new()
	return node

func test_passed():
	var node = Node.new()
	setup_node(node)

func setup_node(_n):
	pass
