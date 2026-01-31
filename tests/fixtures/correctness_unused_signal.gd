extends Node

# Should warn: signal never referenced
signal unused_signal

# Should NOT warn: signal emitted
signal used_signal

# Should NOT warn: signal connected
signal connected_signal

func _ready():
	used_signal.emit()
	connected_signal.connect(_on_connected)

func _on_connected():
	pass
