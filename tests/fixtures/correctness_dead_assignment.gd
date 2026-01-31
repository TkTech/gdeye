extends Node

func dead_in_if_else():
	var x = 1
	if true:
		x = 2
	else:
		x = 3
	print(x)

func dead_simple():
	var y = 10
	y = 20
	print(y)

func not_dead_used_before_reassign():
	var z = 5
	print(z)
	z = 10
	print(z)

func not_dead_conditional():
	var w = 1
	if true:
		w = 2
	print(w)

func not_dead_used_in_elif():
	var chance = 0.0
	if true:
		chance = 0.20
	elif true:
		chance = 0.12
	if randf() < 0.5:
		pass
	elif randf() < chance:
		pass

func dead_reassignment():
	var counter = 0
	counter = 1  # Dead: both branches below overwrite counter
	if true:
		counter = 2
	else:
		counter = 3
	print(counter)
