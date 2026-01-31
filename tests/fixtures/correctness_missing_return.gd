extends Node

func get_value() -> int:
	print("hello")
	# Missing return!

func get_name() -> String:
	return "hello"

func conditional_return(x) -> int:
	if x > 0:
		return 1
	# Missing return on else path!

func full_coverage(x) -> int:
	if x > 0:
		return 1
	else:
		return -1

func no_return_type():
	print("no annotation, no problem")

func void_return() -> void:
	print("void is fine without return")

func nested_if(x, y) -> int:
	if x > 0:
		if y > 0:
			return 1
		else:
			return 2
	else:
		return 3

# Match with all branches returning (should NOT flag)
func match_all_return(x: int) -> String:
	match x:
		0:
			return "zero"
		1:
			return "one"
		_:
			return "other"

# Match without catch-all (should flag missing-return)
func match_no_catchall(x: int) -> String:
	match x:
		0:
			return "zero"
		1:
			return "one"

# Elif chain with all branches returning (should NOT flag)
func elif_all_return(x: int) -> int:
	if x > 10:
		return 1
	elif x > 5:
		return 2
	elif x > 0:
		return 3
	else:
		return 4

# Elif chain missing else (should flag missing-return)
func elif_missing_else(x: int) -> int:
	if x > 10:
		return 1
	elif x > 5:
		return 2
