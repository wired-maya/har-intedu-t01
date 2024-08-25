shader_type canvas_item;
render_mode unshaded, blend_disabled;

uniform sampler2D pattern_texture: filter_nearest; // texture used for the dithering pattern
uniform sampler2D palette_texture: filter_nearest; // Needs to be in increasing luminosity on x axis, and only 1 pixel tall
uniform sampler2D screen_texture: hint_screen_texture, repeat_disable, filter_nearest;

// Simple dithering shader
void fragment() {
	vec3 colour = texture(screen_texture, UV).rgb;

	// Adjust colour acccording to dithering pattern, resulting in a gradient
	float pattern_size = float(textureSize(pattern_texture, 0).x);
	float pattern_x = FRAGCOORD.x - pattern_size * floor(FRAGCOORD.x / pattern_size);
	float pattern_y = FRAGCOORD.y - pattern_size * floor(FRAGCOORD.y / pattern_size);
	vec3 dither_val = texture(pattern_texture, vec2(pattern_x / pattern_size, pattern_y / pattern_size)).rgb;
	colour = colour + dither_val - 0.5; // - 0.5 normalizes value

	// Calculate pixel luminosity (https://stackoverflow.com/questions/596216/formula-to-determine-brightness-of-rgb-color)
	float lum = (colour.r * 0.299) + (colour.g * 0.587) + (colour.b * 0.114);
	clamp(lum, 0.0, 1.0);

	int palette_size = textureSize(palette_texture, 0).x;

	// Convert luminosity to palette colour
	vec3 dithered_colour = texture(palette_texture, vec2(lum, 0.5)).rgb;

	COLOR.rgb = dithered_colour;
}