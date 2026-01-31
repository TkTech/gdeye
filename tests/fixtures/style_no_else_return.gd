extends Node

func with_else_return(x):
	if x > 0:
		return true
	else:
		return false

func with_else_break():
	for i in range(10):
		if i > 5:
			break
		else:
			print(i)

func with_else_continue():
	for i in range(10):
		if i > 5:
			continue
		else:
			print(i)

func no_else_return(x):
	if x > 0:
		return true
	return false

func no_return_in_if(x):
	if x > 0:
		print("positive")
	else:
		print("not positive")
