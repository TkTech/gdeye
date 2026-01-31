extends Node

func test_comparisons():
	var x = 10
	var y = 20

	# Should warn: comparing with itself
	if x == x:
		pass

	# Should warn: not-equal with itself
	if x != x:
		pass

	# Should warn: less-than with itself
	if x < x:
		pass

	# Should NOT warn: different variables
	if x == y:
		pass

	# Should NOT warn: different expressions
	if x < y:
		pass

	# Should warn: complex expression compared with itself
	if x + y == x + y:
		pass

	# Should NOT warn: function calls may return different values
	if get_value() == get_value():
		pass

func get_value():
	return randi()
