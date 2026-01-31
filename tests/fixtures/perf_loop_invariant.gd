extends Node

# Test cases for perf/loop-invariant rule

# SHOULD WARN: Dictionary literal with constant values in loop
func test_dict_invariant():
	for i in range(10):
		var config = {"key": "value", "enabled": true}
		print(config)

# SHOULD WARN: Large array literal with constant values in loop
func test_array_invariant():
	for i in range(10):
		var items = [1, 2, 3, 4, 5]
		print(items)

# SHOULD NOT WARN: Array depends on loop variable
func test_array_depends_on_loop_var():
	for i in range(10):
		var items = [i, i + 1, i + 2]
		print(items)

# SHOULD NOT WARN: Dictionary depends on loop variable
func test_dict_depends_on_loop_var():
	for i in range(10):
		var config = {"index": i}
		print(config)

# SHOULD NOT WARN: Function call (could have side effects)
func test_function_call_not_flagged():
	for i in range(10):
		var result = expensive_calculation()
		print(result)

# SHOULD NOT WARN: Method call (could have side effects)
func test_method_call_not_flagged():
	var rng = RandomNumberGenerator.new()
	for i in range(10):
		var value = rng.randf_range(0, 1)
		print(value)

# SHOULD NOT WARN: Small array (2 or fewer elements)
func test_small_array_not_flagged():
	for i in range(10):
		var pair = [1, 2]
		print(pair)

# SHOULD NOT WARN: Empty dictionary (intentionally fresh each iteration)
func test_empty_dict_not_flagged():
	for i in range(10):
		var data = {}
		data[i] = "value"
		print(data)

# SHOULD NOT WARN: Empty array
func test_empty_array_not_flagged():
	for i in range(10):
		var items = []
		items.append(i)
		print(items)

# SHOULD NOT WARN: Variable modified in loop
func test_modified_var_not_flagged():
	var count = 0
	for i in range(10):
		var data = {"count": count}
		count += 1
		print(data)

# SHOULD WARN: While loop with invariant dict
func test_while_loop_invariant():
	var i = 0
	while i < 10:
		var settings = {"mode": "fast", "debug": false}
		print(settings)
		i += 1

# Helper function
func expensive_calculation() -> int:
	return 42
