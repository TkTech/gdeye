extends Node

func test_match_uses_variable():
	var value = get_input()
	var threshold = 10
	match value:
		0:
			return threshold + 1
		1:
			return threshold * 2
		_:
			return threshold

func test_match_subject_used():
	var subject = calculate()
	match subject:
		"a":
			print("got a")
		_:
			print("other")

func test_genuinely_unused():
	var unused_in_match = 5
	var used = 10
	match used:
		5:
			print("five")
		_:
			print("other")

func get_input():
	return 0

func calculate():
	return "a"
