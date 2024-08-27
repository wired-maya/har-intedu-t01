shader_type canvas_item;
render_mode unshaded, blend_disabled;

uniform sampler2D screen_texture: repeat_disable, filter_nearest;

void fragment() {
	// Simply pass through colour
	COLOR = texture(screen_texture, UV);
}