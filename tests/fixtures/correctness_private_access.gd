extends Node

var _private_var = 10
var public_var = 20

func _private_method():
	pass

func public_method():
	# Accessing own private members is fine
	print(_private_var)
	_private_method()
	self._private_method()
