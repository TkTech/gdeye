extends Control

# Correct: matches signal parameter count
func _on_died() -> void:
	pass

# Correct: matches damage_taken(amount) - 1 param
func _on_damage_taken(amount: int) -> void:
	pass

# WRONG: health_changed passes 2 args but handler takes 0
func _on_health_changed_wrong() -> void:
	pass

# Correct: matches health_changed(new_health, max_health) - 2 params
func _on_health_changed_ok(new_health: int, max_health: int) -> void:
	pass
