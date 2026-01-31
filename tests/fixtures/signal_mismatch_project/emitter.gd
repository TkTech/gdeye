extends Node2D

signal health_changed(new_health, max_health)
signal died
signal damage_taken(amount)

func take_damage(amount: int) -> void:
	emit_signal("damage_taken", amount)
	emit_signal("health_changed", 50, 100)
