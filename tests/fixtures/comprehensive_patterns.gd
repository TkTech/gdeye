extends Node2D

# Enum with values
enum Direction { UP, DOWN, LEFT, RIGHT }

# Constants
const MAX_SPEED: float = 100.0
const MIN_SPEED := 10.0

# Inner class
class InnerHelper:
    var value: int = 0
    func get_value() -> int:
        return value

# Signals
signal health_changed(new_health: int)
signal game_over

# Member variables
var speed: float = 50.0
var direction: int = Direction.UP
var helper := InnerHelper.new()

# Function with while loop
func count_down(start: int) -> int:
    var count = start
    while count > 0:
        count -= 1
    return count

# Function with for loop
func sum_array(arr: Array) -> int:
    var total = 0
    for item in arr:
        total += item
    return total

# Function with for range
func repeat_action(times: int) -> int:
    var result = 0
    for i in range(times):
        result += i
    return result

# Function with match and multiple patterns
func describe_direction(dir: int) -> String:
    match dir:
        Direction.UP:
            return "up"
        Direction.DOWN:
            return "down"
        Direction.LEFT:
            return "left"
        _:
            return "other"

# Function that uses self explicitly
func get_current_speed() -> float:
    return self.speed

# Function with typed default parameter
func move_with_speed(delta: float, multiplier: float = 1.0) -> Vector2:
    var velocity = Vector2(speed * multiplier, 0)
    return velocity * delta

# Unused signal (should be flagged)
signal unused_signal_test

# Shadowed variable
var shadow_target: int = 5
func shadow_example():
    var shadow_target = 10
    print(shadow_target)

# Negative literal initializers
var neg_int = -5
var neg_float = -3.14

# Variable initialized with self.method()
@onready var viewport_ref = self.get_viewport()

# Unreachable code after break/continue
func loop_with_unreachable():
    for i in range(10):
        if i == 5:
            break
            var never_reached = 0
        if i == 3:
            continue
            var also_never_reached = 0
