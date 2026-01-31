extends Node

# Should warn: code after return
func after_return():
	var x = 1
	return x
	var y = 2
	print(y)

# Should warn: code after break in loop
func after_break():
	for i in range(10):
		break
		print(i)

# Should warn: code after continue in loop
func after_continue():
	for i in range(10):
		continue
		print(i)

# Should NOT warn: return in if branch doesn't make the rest unreachable
func conditional_return(flag):
	if flag:
		return 1
	var x = 2
	return x

# Should NOT warn: break in if branch doesn't affect outer block
func conditional_break():
	for i in range(10):
		if i > 5:
			break
		print(i)

# Should NOT warn: no terminator
func normal():
	var x = 1
	var y = 2
	return x + y

# Should warn: nested unreachable
func nested_unreachable():
	if true:
		return
		print("dead")
	print("alive")
