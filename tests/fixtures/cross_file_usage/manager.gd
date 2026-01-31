extends Node

var score: int = 0

# This is unused but since it's an autoload, the unused-function check is skipped
# (no class_name, but accessed via autoload)
func add_score(points: int) -> void:
	score += points
