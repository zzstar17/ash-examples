#version 450

layout(push_constant) uniform PushConstantData {
  mat4 matrix;
  vec2 tex_offset;
  vec2 tex_size;
} pc;

// vertex
layout(location = 0) in vec3 vertex_pos;
layout(location = 1) in vec3 vertex_normal;
layout(location = 2) in vec2 tex_coords;

layout(location = 0) out vec2 out_tex_coords;

void main() {
  gl_Position = pc.matrix * vec4(vertex_pos, 1.0);
  
  out_tex_coords = tex_coords * pc.tex_size + pc.tex_offset;
}
