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

func test_assigned_then_added():
	var node = Node3D.new()
	var other = node
	add_child(other)  # node is added via alias

func test_assigned_to_member_field():
	var node = Node3D.new()
	asteroid_mesh = node
	add_child(asteroid_mesh)  # node added via member field alias

func test_alias_without_sink():
	var node = Node3D.new()
	var other = node
	# alias exists but never added to tree — still orphaned
