extends Node

# Should warn: variable used before initialization
func test_uninitialized_use():
	var x
	print(x)  # x is uninitialized here
	x = 10

# Should warn: variable used in one branch before init
func test_conditional_uninitialized():
	var result
	if true:
		result = 1
	print(result)  # result might be uninitialized if condition is false

# Should NOT warn: variable initialized before use
func test_initialized():
	var x = 10
	print(x)

# Should NOT warn: variable declared with type has default value
func test_typed_declaration():
	var x: int
	print(x)  # int defaults to 0

# Should NOT warn: underscore prefix means intentionally unused
func test_underscore_prefix():
	var _unused
	pass

# Should NOT warn: variable initialized in all branches
func test_all_branches_initialized():
	var x
	if true:
		x = 1
	else:
		x = 2
	print(x)

# Should NOT warn: variable used inside lambda (different scope)
func test_lambda_scope():
	var outer = 10
	var cb = func():
		var inner
		print(inner)  # lambda has its own scope
	cb.call()

# Should NOT warn: match default case returns early, so variable is always initialized
func test_match_early_return(value: int):
	var result
	match value:
		1:
			result = "one"
		2:
			result = "two"
		_:
			return "default"
	print(result)
	return result

# Should NOT warn: if-elif-else with early return in else prevents uninitialized use
func test_if_early_return(value: int):
	var category
	if value > 100:
		category = "large"
	elif value > 10:
		category = "medium"
	else:
		return "tiny"
	print(category)
	return category

# Should NOT warn: loop with continue prevents reaching the use point with uninitialized var
func test_loop_continue():
	for i in range(10):
		var status
		if i > 5:
			status = "high"
		elif i > 2:
			status = "medium"
		else:
			continue
		print(status)

# Should NOT warn: guard flag ensures variable is always initialized before use.
# First iteration: flag is false, so the variable gets assigned and flag becomes true.
# Subsequent iterations: flag is true, so assignment is skipped, but variable is
# already initialized from the first iteration.
func test_loop_guard_flag():
	var cached_value
	var is_loaded = false
	for item in [1, 2, 3]:
		if not is_loaded:
			cached_value = item * 10
			is_loaded = true
		print(cached_value)

# Should NOT warn: same immutable condition guards both assignment and use.
# If the condition is true, the variable is assigned and later used.
# If the condition is false, neither the assignment nor the use is reached.
func test_correlated_condition(flag: bool):
	var special_value
	if flag:
		special_value = 42
	var other = 0
	if flag:
		print(special_value)
