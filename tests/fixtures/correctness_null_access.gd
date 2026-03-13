extends Node

func test_chained():
	get_node_or_null("Child").show()
	find_child("Missing").queue_free()
	get_child(0).visible = false

func test_get_node_not_flagged():
	# get_node throws on missing nodes, never returns null
	get_node("Child").visible = false

func test_dollar():
	$Child.visible = false
	$Missing/Path.show()

func test_safe():
	var node = get_node_or_null("Child")
	if node:
		node.visible = false

# These should NOT be flagged (guarded access patterns)
func test_guarded_dollar():
	if $Child:
		$Child.visible = false  # guarded by if $Child:

func test_guarded_has_node():
	if has_node("Child"):
		$Child.show()  # guarded by has_node check

func test_guarded_is_instance_valid():
	var node = get_node_or_null("Child")
	if is_instance_valid(node):
		node.visible = false  # guarded by is_instance_valid

func test_guarded_null_comparison():
	var node = get_node_or_null("Child")
	if node != null:
		node.visible = false  # guarded by != null check

func test_guarded_boolean_and():
	var node = get_node_or_null("Child")
	if node and node.visible:
		node.show()  # guarded by node in boolean and
