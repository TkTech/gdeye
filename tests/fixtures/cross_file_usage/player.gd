class_name Player
extends CharacterBody3D

signal hit(damage: int)
signal died

# Used via cross-file attribute access (game.gd accesses player.health)
var health: int = 100

# Never used anywhere - should be flagged
var unused_stat: int = 0

# Used via scene property assignment
var speed: float = 5.0

func take_damage(amount: int) -> void:
	health -= amount
	hit.emit(amount)
	if health <= 0:
		died.emit()

func get_health() -> int:
	return health

# Never called - but class_name means we skip unused-function check
func unused_helper() -> void:
	pass
