#version 450

layout(push_constant) uniform PushConstantData {
  mat4 projection_view;
} pc;

// vertex
layout(location = 0) in vec3 vertex_pos;
layout(location = 1) in vec3 _vertex_normal;
layout(location = 2) in vec2 _tex_coords;

layout(location = 0) out vec3 out_tex_coords;

void main() {
  out_tex_coords = vertex_pos;
  
  vec4 pos = pc.projection_view * vec4(vertex_pos, 1.0);
  gl_Position = pos.xyww;
}
