extends Node

enum State { IDLE, RUNNING, JUMPING, FALLING }

var current_state: State = State.IDLE

func check_state() -> void:
	# Should warn: missing FALLING
	match current_state:
		State.IDLE:
			print("idle")
		State.RUNNING:
			print("running")
		State.JUMPING:
			print("jumping")

func check_state_complete() -> void:
	# Should NOT warn: all variants covered
	match current_state:
		State.IDLE:
			print("idle")
		State.RUNNING:
			print("running")
		State.JUMPING:
			print("jumping")
		State.FALLING:
			print("falling")

func check_state_wildcard() -> void:
	# Should NOT warn: has wildcard
	match current_state:
		State.IDLE:
			print("idle")
		_:
			print("other")
