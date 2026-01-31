extends Node

var member = 10

func test_standalone():
	# Should warn: standalone variable reference
	member

	# Should warn: standalone arithmetic
	1 + 2

	# Should warn: standalone comparison
	member == 10

	# Should NOT warn: function calls have side effects
	print("hello")

	# Should NOT warn: method calls have side effects
	get_tree().quit()

	# Should NOT warn: assignments have side effects
	var x = 5
	x += 1

	# Should warn: standalone attribute access (no call)
	self.member

	# Should NOT warn: await has side effects
	await get_tree().create_timer(1.0).timeout
