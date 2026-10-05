#version 450

layout(location = 0) in vec2 tex_coords;
layout(location = 0) out vec4 out_color;

layout(binding = 0) uniform sampler2D tex_sampler;

void main() {
  // vec4 color = textureLod(tex_sampler, tex_coords, 0);
  vec4 color = texture(tex_sampler, tex_coords);
  // if (color.a < 0.1) {
  //   discard;
  // }

  out_color = color;
}
