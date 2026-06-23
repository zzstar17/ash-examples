#version 450

layout(push_constant) uniform PushConstantData {
  vec2 position;
  vec2 ratio; // relative width and height
} pc;

// vertex
layout(location = 0) in vec2 vertex_pos;
layout(location = 1) in vec2 tex_coords;

layout(location = 0) out vec2 out_tex_coords;

void main() {
  vec2 final_pos = (vertex_pos * pc.ratio) + ((pc.position - 0.5) * 2);
  gl_Position = vec4(final_pos.x, final_pos.y, 1.0, 1.0);
  
  out_tex_coords = tex_coords;
}
