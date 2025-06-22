#version 450

// Input from vertex shader
layout (location = 0) in vec4 v_color;

// Output
layout (location = 0) out vec4 o_color;

void main() {
    o_color = v_color;
}