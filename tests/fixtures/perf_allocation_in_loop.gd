extends Node

# Allocation in loops - should warn

func bad_array_in_for():
    for i in range(10):
        var arr = Array()  # WARN: perf/allocation-in-loop
        arr.append(i)

func bad_dictionary_in_while():
    var i = 0
    while i < 10:
        var dict = Dictionary()  # WARN: perf/allocation-in-loop
        dict["key"] = i
        i += 1

func bad_object_new_in_loop():
    for i in range(10):
        var node = Node.new()  # OK - .new() in loops handled by process-allocation for hot paths
        add_child(node)

func bad_array_literal_in_loop():
    for i in range(10):
        var items = [1, 2, 3, 4, 5, 6]  # WARN: perf/allocation-in-loop (5+ elements)
        print(items)

func bad_dict_literal_in_loop():
    for i in range(10):
        var data = {"key1": "value1", "key2": "value2", "key3": "value3"}  # WARN: perf/allocation-in-loop (3+ pairs)
        print(data)

func bad_custom_class_new():
    for i in range(10):
        var bullet = Bullet.new()  # OK - .new() in loops handled by process-allocation for hot paths
        bullet.shoot()

# These should NOT warn

func ok_allocation_outside_loop():
    var arr = Array()  # OK - outside loop
    for i in range(10):
        arr.append(i)
    print(arr)

func ok_small_array_literal():
    for i in range(10):
        var pair = [i, i + 1]  # OK - small array
        print(pair)

func ok_empty_dict_literal():
    for i in range(10):
        var d = {}  # OK - empty dict
        d["key"] = i

func ok_cheap_value_types():
    for i in range(10):
        var pos = Vector2(i, i)  # OK - cheap value type
        var color = Color(1, 0, 0)  # OK - cheap value type
        print(pos, color)

func ok_nested_function():
    for i in range(10):
        var inner = func():
            var node = Node.new()  # OK - in nested function
            return node
        print(inner)
