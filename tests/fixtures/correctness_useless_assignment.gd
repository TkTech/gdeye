extends Node

func test_useless():
	var counter = 0
	counter = 1  # Dead: both branches below overwrite
	if true:
		counter = 2
	else:
		counter = 3
	print(counter)

func test_ok():
	var y = 5
	print(y)
	y = 10
	print(y)

# Test: variable assigned in match arm, used in method call - should NOT warn
func test_match_then_method_call(value: String):
	var mode_idx := 0
	match value:
		"a":
			mode_idx = 0
		"b":
			mode_idx = 1
		"c":
			mode_idx = 2
	some_object.select(mode_idx)  # mode_idx IS used here

# Test: variable assigned in match arm, used in function call - should NOT warn
func test_match_then_function_call(value: String):
	var result := ""
	match value:
		"x":
			result = "found x"
		"y":
			result = "found y"
	print(result)  # result IS used here

# Test: variable assigned in match but never used - SHOULD warn
func test_match_unused(value: String):
	var unused := 0
	match value:
		"a":
			unused = 1
		"b":
			unused = 2
	# unused is never read - this should warn
