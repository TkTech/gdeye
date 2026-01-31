extends Node

func test_duplicate_keys():
	var d = {
		"name": "Alice",
		"age": 30,
		"name": "Bob",
	}

	var d2 = {
		1: "one",
		2: "two",
		1: "uno",
	}

	var d3 = {
		key: "value1",
		other: "value2",
		key: "value3",
	}

func test_no_duplicates():
	var d = {
		"a": 1,
		"b": 2,
		"c": 3,
	}
