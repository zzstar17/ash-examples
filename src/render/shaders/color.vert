#version 450

layout(push_constant) uniform PushConstantData {
  mat4 matrix;
  vec4 color;
} pc;

// vertex
layout(location = 0) in vec3 vertex_pos;
layout(location = 1) in vec3 _vertex_normal;
layout(location = 2) in vec2 _tex_coords;

layout(location = 0) out vec4 out_color;

void main() {
  gl_Position = pc.matrix * vec4(vertex_pos, 1.0);
  
  out_color = pc.color;
}
