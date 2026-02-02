extends Node

func dead_in_if_else():
	var x = 1
	if true:
		x = 2
	else:
		x = 3
	print(x)

func dead_simple():
	var y = 10
	y = 20
	print(y)

func not_dead_used_before_reassign():
	var z = 5
	print(z)
	z = 10
	print(z)

func not_dead_conditional():
	var w = 1
	if true:
		w = 2
	print(w)

func not_dead_used_in_elif():
	var chance = 0.0
	if true:
		chance = 0.20
	elif true:
		chance = 0.12
	if randf() < 0.5:
		pass
	elif randf() < chance:
		pass

func dead_reassignment():
	var counter = 0
	counter = 1  # Dead: both branches below overwrite counter
	if true:
		counter = 2
	else:
		counter = 3
	print(counter)

# Should NOT warn: conditional assignment with default fallback pattern
# This is a common pattern: set default, conditionally override, use result
func not_dead_conditional_fallback():
	var cam_pos = Vector3.ZERO
	if camera and is_instance_valid(camera):
		cam_pos = camera.global_position
	# cam_pos is used here - either default or overwritten value
	print(cam_pos)

# Should NOT warn: conditional assignment where value IS used if condition is false
func not_dead_conditional_assignment_used():
	var fleet_pos = get_fleet_position()
	if fleet_pos == Vector3.ZERO:
		fleet_pos = get_fallback_position()
	# fleet_pos is used - either original or fallback
	print(fleet_pos)

# Should warn: conditional assignment where initial value is NEVER read
func dead_conditional_always_overwritten():
	var value = 100  # This initial value is never read
	if condition_a():
		value = 200
	else:
		value = 300  # All paths overwrite before use
	print(value)

# Helper stubs for the tests
var camera = null
func is_instance_valid(_node) -> bool:
	return true
func get_fleet_position() -> Vector3:
	return Vector3.ZERO
func get_fallback_position() -> Vector3:
	return Vector3.ONE
func condition_a() -> bool:
	return true
