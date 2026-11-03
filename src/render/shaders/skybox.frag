#version 450

layout(location = 0) in vec3 tex_coords;
layout(location = 0) out vec4 out_color;

layout(binding = 0) uniform samplerCube skybox;

void main() {
  out_color = texture(skybox, tex_coords);
  // out_color = vec4(0.0, 0.0, 0.0, 1.0);
}
