extends Node

func _process(_delta):
	if Input.is_action_pressed("move_left"):
		pass
	if Input.is_action_pressed("nonexistent_action"):
		pass
	if Input.is_action_just_pressed("jump"):
		pass
	if Input.is_action_just_pressed("bad_action"):
		pass
	var axis = Input.get_axis("move_left", "move_right")
	var bad_axis = Input.get_axis("go_left", "go_right")
	if Input.is_action_pressed("ui_accept"):
		pass
