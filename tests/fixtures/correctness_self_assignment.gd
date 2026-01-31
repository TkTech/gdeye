extends Node

var health = 100

func test_self_assign():
	var x = 5
	x = x
	health = health
	self.health = self.health

func test_ok():
	var x = 5
	var y = x
	x = y
	x += x
	x -= x
