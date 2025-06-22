#version 450

// Texture sampler
layout (set = 2, binding = 0) uniform sampler2D u_texture;

// Inputs from vertex shader
layout (location = 0) in vec2 v_tex_coord;
layout (location = 1) in vec4 v_color;

// Output
layout (location = 0) out vec4 o_color;

void main() {
    vec4 tex_color = texture(u_texture, v_tex_coord);
    // Multiply texture by vertex color for tinting
    o_color = tex_color * v_color;
}