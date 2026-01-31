extends Node

# Using autoload singletons (defined in project.godot as GameManager and EventBus)
# These should NOT produce "undeclared identifier" warnings (if such a rule existed)
# and should not interfere with local variable detection.

var local_var = 10

func _ready():
	GameManager.start()
	EventBus.emit("ready")
	print(local_var)

func has_unused():
	var unused_var = 5
	pass
