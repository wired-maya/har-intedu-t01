extends AnimationPlayer

func _on_start_label_focus_exited() -> void:
	play_backwards("start_rect_anim")

func _on_start_label_focus_entered() -> void:
	play("start_rect_anim")

func _on_start_label_mouse_exited() -> void:
	play_backwards("start_rect_anim")

func _on_start_label_mouse_entered() -> void:
	play("start_rect_anim")