shader_type canvas_item;
render_mode unshaded, blend_disabled;

// Kernel for sharpening image
const int kernel_len = 9;
const float kernel[kernel_len] = float[](
	-1.0, -1.0, -1.0,
	-1.0,  9.0, -1.0,
	-1.0, -1.0, -1.0
);
const vec2 kernel_offsets[kernel_len] = vec2[](
	vec2(-1.0,  1.0), // Top left
	vec2( 0.0,  1.0), // Top middle
	vec2( 1.0,  1.0), // Top right
	vec2(-1.0,  0.0), // Middle left
	vec2( 0.0,  0.0), // Middle middle (centre pixel)
	vec2( 1.0,  0.0), // Middle right
	vec2(-1.0, -1.0), // Bottom left
	vec2( 0.0, -1.0), // Bottom left
	vec2( 1.0, -1.0)  // Bottom left
);

uniform sampler2D palette_texture: filter_nearest; // Needs to be in increasing luminosity on x axis, and only 1 pixel tall
uniform sampler2D screen_texture: hint_screen_texture, repeat_disable, filter_nearest;

// Simple dithering shader
void fragment() {
	//vec3 colour = texture(screen_texture, UV).rgb;

	// Adjust kernel offsets to pixel size
	vec2 kernel_offsets_adj[kernel_len] = kernel_offsets;
	for(int i = 0; i < kernel_len; i++) {
		kernel_offsets_adj[i].x *= SCREEN_PIXEL_SIZE.x;
		kernel_offsets_adj[i].y *= SCREEN_PIXEL_SIZE.y;
	}

	// Get samples of screen around current fragment
	vec3 screen_samples[kernel_len];
	for(int i = 0; i < kernel_len; i++) {
		screen_samples[i] = vec3(texture(screen_texture, UV + kernel_offsets_adj[i]).rgb);
	}
	vec3 colour = vec3(0.0);
	for(int i = 0; i < kernel_len; i++) {
		colour += screen_samples[i] * kernel[i];
	}

	// Calculate pixel luminosity (https://stackoverflow.com/questions/596216/formula-to-determine-brightness-of-rgb-color)
	float lum = (colour.r * 0.299) + (colour.g * 0.587) + (colour.b * 0.114);
	clamp(lum, 0.0, 1.0);

	// Convert luminosity to palette colour
	vec3 dithered_colour = texture(palette_texture, vec2(lum, 0.5)).rgb;

	//COLOR.rgb = dithered_colour;
	COLOR.rgb = dithered_colour;
}