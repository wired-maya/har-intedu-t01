extends AnimationPlayer

func _on_option_label_mouse_exited() -> void:
	play_backwards("option_rect_anim")

func _on_option_label_mouse_entered() -> void:
	play("option_rect_anim")

func _on_option_label_focus_exited() -> void:
	play_backwards("option_rect_anim")

func _on_option_label_focus_entered() -> void:
	play("option_rect_anim")
