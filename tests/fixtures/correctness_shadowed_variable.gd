extends Node

var health = 100
var speed = 5.0

func take_damage(amount):
	var health = 50  # shadows member variable
	print(health - amount)

func move_player(direction):
	var speed = 10.0  # shadows member variable
	print(direction * speed)

func no_shadow():
	var local_only = 42
	print(local_only)

func shadow_param(value):
	var value = 10  # shadows parameter
	print(value)

# Static functions cannot access self, so locals should NOT shadow members
static func static_no_shadow():
	var health = 999  # NOT a shadow - static func can't access self.health
	print(health)
