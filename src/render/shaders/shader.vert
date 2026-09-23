#version 450

layout(push_constant) uniform PushConstantData {
  mat4 matrix;
} pc;

// vertex
layout(location = 0) in vec2 vertex_pos;
layout(location = 1) in vec2 tex_coords;

layout(location = 0) out vec2 out_tex_coords;

void main() {
  gl_Position = pc.matrix * vec4(vertex_pos, 0.0, 1.0);
  
  out_tex_coords = tex_coords;
}
