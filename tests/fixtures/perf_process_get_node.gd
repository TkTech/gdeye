extends Node

@onready var player = $Player

func _process(delta):
	# Should warn: get_node in _process
	var node = get_node("Player")
	node.visible = true

	# Should warn: get_node_or_null in _process
	var maybe = get_node_or_null("Enemy")
	if maybe:
		maybe.queue_free()

	# Should NOT warn: using cached @onready reference
	player.position += Vector3.UP * delta

func _input(event):
	# Should warn: get_node in _input
	get_node("UI/Label").text = "pressed"

func some_function():
	# Should NOT warn: not in a process function
	var node = get_node("Something")
	print(node)
