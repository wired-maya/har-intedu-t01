shader_type canvas_item;
render_mode unshaded, blend_disabled;

uniform sampler2DArray pattern_texture: filter_nearest; // texture used for the dithering pattern
uniform sampler2D palette_texture: filter_nearest; // Needs to be in increasing luminosity on x axis, and only 1 pixel tall
uniform sampler2D screen_texture: hint_screen_texture, repeat_disable, filter_nearest;
uniform int pattern_index;

// Simple dithering shader
void fragment() {
	vec4 colour = texture(screen_texture, UV);
	int pattern_size = textureSize(pattern_texture, 0).x;
	bool should_pass_through = false;

	// Pass through colour if already in palette
	for(int i = 0; i < pattern_size; i++) {
		should_pass_through = texture(palette_texture, vec2(0.0)) == colour;
	}

	if(should_pass_through) {
		COLOR.rgb = colour.rgb;
	} else {
		// Adjust colour acccording to dithering pattern, resulting in a gradient
		float pattern_size_f = float(pattern_size);
		float pattern_x = FRAGCOORD.x - pattern_size_f * floor(FRAGCOORD.x / pattern_size_f);
		float pattern_y = FRAGCOORD.y - pattern_size_f * floor(FRAGCOORD.y / pattern_size_f);
		vec2 tex_coords = vec2(pattern_x / pattern_size_f, pattern_y / pattern_size_f);
		vec3 dither_val = texture(pattern_texture, vec3(tex_coords, float(pattern_index))).rgb;
		vec3 colour_adj = colour.rgb + dither_val - 0.5; // normalizes value

		// Calculate pixel luminosity (https://stackoverflow.com/questions/596216/formula-to-determine-brightness-of-rgb-color)
		float lum = (colour_adj.r * 0.299) + (colour_adj.g * 0.587) + (colour_adj.b * 0.114);
		lum *= colour.a; // Account for transparency
		clamp(lum, 0.0, 1.0);

		// Convert luminosity to palette colour
		vec3 dithered_colour = texture(palette_texture, vec2(lum, 0.0)).rgb;

		COLOR.rgb = dithered_colour;
	}

	COLOR.a = 1.0;
}