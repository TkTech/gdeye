extends Node

var scene_a = preload("res://scenes/enemy.tscn")
var scene_b = preload("res://scenes/enemy.tscn")

func _ready():
	var script = load("res://scripts/helper.gd")
	var script2 = load("res://scripts/helper.gd")

func test_no_dup():
	var a = load("res://scripts/a.gd")
	var b = load("res://scripts/b.gd")
