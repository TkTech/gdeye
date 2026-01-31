extends Node

var player: Player

func _ready() -> void:
	player = Player.new()
	add_child(player)
	player.hit.connect(_on_player_hit)

func _on_player_hit(damage: int) -> void:
	print("Player took ", damage, " damage, health: ", player.health)
	if player.health <= 0:
		_game_over()

func _game_over() -> void:
	player.take_damage(0)
	print("Game over")
