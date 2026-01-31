extends Node

# Should warn: pass is unnecessary here
func has_code_and_pass():
	var x = 1
	pass
	print(x)

# Should NOT warn: pass is the only statement
func only_pass():
	pass

# Should warn: pass after a return
func pass_after_return():
	return 42
	pass

# Should NOT warn: pass in empty if body
func pass_in_empty_if():
	if true:
		pass
	else:
		print("hello")
