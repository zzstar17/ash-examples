#version 450

layout(push_constant) uniform PushConstantData {
  vec2 position;
  vec2 ratio; // relative width and height
} pc;

// vertex
layout(location = 0) in vec2 vertex_pos;
layout(location = 1) in vec2 tex_coords;

// instance
layout(location = 2) in vec2 instance_pos;
layout(location = 3) in vec2 instance_vel;

layout(location = 0) out vec2 out_tex_coords;

void main() {
  float x = (instance_pos.x + vertex_pos.x) * pc.ratio.x;//  + pc.position.x;
  float y = (instance_pos.y + vertex_pos.y) * pc.ratio.y;//  + pc.position.y;
  gl_Position = vec4(x, y, 1.0, 1.0);
  
  out_tex_coords = tex_coords;
}
