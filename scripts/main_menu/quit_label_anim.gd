extends AnimationPlayer

func _on_quit_label_mouse_exited() -> void:
	play_backwards("quit_rect_anim")

func _on_quit_label_mouse_entered() -> void:
	play("quit_rect_anim")

func _on_quit_label_focus_exited() -> void:
	play_backwards("quit_rect_anim")

func _on_quit_label_focus_entered() -> void:
	play("quit_rect_anim")
