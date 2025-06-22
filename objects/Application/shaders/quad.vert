#version 450

// Vertex attributes
layout (location = 0) in vec2 a_pos;
layout (location = 1) in vec2 a_tex_coord;
layout (location = 2) in vec4 a_color;

// Outputs to fragment shader
layout (location = 0) out vec2 v_tex_coord;
layout (location = 1) out vec4 v_color;

// Uniforms that are pushed via push_vertex_uniform_data
layout(set = 1, binding = 0) uniform PushConstants {
    vec2 screen_size;
};

void main() {
    // Convert from pixel coordinates to NDC (-1 to 1)
    vec2 ndc_pos = (a_pos / screen_size) * 2.0 - 1.0;
    // Flip Y axis (SDL uses top-left origin, NDC uses bottom-left)
    ndc_pos.y = -ndc_pos.y;
    
    gl_Position = vec4(ndc_pos, 0.0, 1.0);
    v_tex_coord = a_tex_coord;
    v_color = a_color;
}