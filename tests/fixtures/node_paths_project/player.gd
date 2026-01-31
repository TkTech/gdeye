extends CharacterBody2D

@onready var sprite = $Sprite
@onready var camera = $Camera
@onready var collision = $CollisionShape

# These should be flagged as broken
@onready var bad1 = $NonExistentNode
@onready var bad2 = $Sprite/SubChild
@onready var bad3 = get_node("DoesNotExist")

func _ready():
	# Valid paths
	var s = $Sprite
	var c = get_node("Camera")
	var cs = get_node_or_null("CollisionShape")

	# Broken paths
	var nope = $Oops
	var also_nope = get_node("Missing/Node")
