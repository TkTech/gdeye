extends Node

# String concatenation in loops - should warn

func bad_string_concat_for():
    var result = ""
    for i in range(10):
        result += "item"  # WARN: perf/string-concat-in-loop
    print(result)

func bad_string_concat_while():
    var result = ""
    var i = 0
    while i < 10:
        result += "item"  # WARN: perf/string-concat-in-loop
        i += 1
    print(result)

func bad_string_concat_assignment():
    var result = ""
    for i in range(10):
        result = result + "item"  # WARN: perf/string-concat-in-loop
    print(result)

func bad_string_concat_with_variable():
    var result: String = ""
    var item = "test"
    for i in range(10):
        result += item  # WARN: perf/string-concat-in-loop
    print(result)

# These should NOT warn

func ok_array_join():
    var parts: Array[String] = []
    for i in range(10):
        parts.append("item")  # OK - not string concat
    var result = "".join(parts)
    print(result)

func ok_single_concat():
    var result = ""
    result += "item"  # OK - not in a loop
    print(result)

func ok_int_in_loop():
    var sum = 0
    for i in range(10):
        sum += i  # OK - int, not string
    print(sum)

func ok_nested_function():
    for i in range(10):
        var inner = func():
            var result = ""
            result += "item"  # OK - nested function
            return result
        print(inner.call())
