extends Node

signal unused_signal_suppressed # gdeye:ignore

signal unused_signal_not_suppressed

func example(unused_param): # gdeye:ignore
	pass

func example2(unused_param2):
	pass

func suppression_cases():
	# No suppression — should warn
	var unused_no_suppress = 1

	# gdeye:ignore-next-line
	var unused_blanket_next = 2

	var unused_specific_same = 3 # gdeye:ignore correctness/dead-store

	# gdeye:ignore-next-line correctness/dead-store
	var unused_specific_next = 4

	# gdeye:ignore-next-line perf/allocation
	var unused_wrong_rule = 5

	var unused_inline_blanket = 6 # gdeye:ignore

	pass
