extends Node2D

# Type mismatch from literal initializer
var x: String = 42

# Type mismatch from call initializer
var vp: String = get_viewport()

# No mismatch (compatible types)
var speed: float = 10
