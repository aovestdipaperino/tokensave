shader_type spatial;
render_mode blend_mix, depth_draw_opaque, cull_back;

#include "res://shaders/common.gdshaderinc"

uniform sampler2D albedo : source_color;
uniform float roughness_value : hint_range(0.0, 1.0) = 0.5;
uniform vec4 tint_color;
global uniform vec3 global_wind;
instance uniform vec4 instance_tint;

float compute_fresnel(float ndotv, float f0) {
    return f0 + (1.0 - f0) * pow(1.0 - ndotv, 5.0);
}

void fragment() {
    float fresnel = compute_fresnel(dot(NORMAL, VIEW), 0.04);
    ALBEDO = texture(albedo, UV).rgb;
    ROUGHNESS = roughness_value;
    SPECULAR = fresnel;
}
