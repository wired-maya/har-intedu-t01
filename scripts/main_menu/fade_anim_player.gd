extends AnimationPlayer

func _on_quit_label_gui_input(event: InputEvent) -> void:
	if event is InputEventAction:
		if event.pressed and event.action == "ConfirmAction":
			get_tree().quit()

func fade_out() -> void:
	play_backwards("fade_anim")

func fade_in() -> void:
	play("fade_anim")