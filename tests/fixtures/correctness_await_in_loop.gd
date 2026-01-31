extends Node

func test_await_in_for():
	for i in range(10):
		await get_tree().create_timer(1.0).timeout

func test_await_in_while():
	var done = false
	while not done:
		await some_async()
		done = true

func test_await_outside_loop():
	await some_async()
	for i in range(5):
		print(i)

func some_async():
	await get_tree().create_timer(0.1).timeout
