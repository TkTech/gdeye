extends Node

func _process(_delta):
	await get_tree().create_timer(1.0).timeout

func _physics_process(_delta):
	await some_async()

func some_async():
	await get_tree().create_timer(0.5).timeout

func not_a_coroutine():
	print("hello")

func test_await_non_coroutine():
	await not_a_coroutine()

func test_await_coroutine():
	await some_async()
