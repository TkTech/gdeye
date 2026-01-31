extends Node

# Test cases for style/onready-hoist rule

# SHOULD WARN: Member variable initialized with node path without @onready
var label = $Label
var button = $UI/Button

# SHOULD NOT WARN: Already has @onready
@onready var sprite = $Sprite

# SHOULD NOT WARN: Member variable without node path
var counter = 0
var name_str = "test"

# SHOULD NOT WARN: Local variable with node path (inside function)
func test_local():
	var local_label = $Label
	print(local_label)

# SHOULD WARN: Member variable assigned node path in _ready() - hoistable
var player
var enemy: Node

func _ready():
	player = $Player
	enemy = $Enemy
	print("ready")

# SHOULD NOT WARN: Member variable assigned non-node-path in _ready()
var health: int

func _process(_delta):
	pass

# Separate function to test that assignments in other functions aren't hoisted
func other_func():
	# This shouldn't trigger hoist warnings
	pass

# SHOULD NOT WARN: @onready var with type annotation
@onready var typed_sprite: Sprite2D = $TypedSprite

# Test class to ensure inner classes work
class InnerClass:
	var inner_label = $InnerLabel  # SHOULD WARN
