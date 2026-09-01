#version 450

layout(push_constant) uniform PushConstantData {
  vec2 render_dimensions;
  vec2 text_size;
} pc;

//     vertex
// -1.0 to 1.0
layout(location = 0) in vec2 vertex_pos;
layout(location = 1) in vec2 tex_coords;

// instance
// 0.0 to 1.0
layout(location = 2) in vec2 instance_pos;
layout(location = 3) in vec2 instance_vel;

layout(location = 0) out vec2 out_tex_coords;

const vec2 ferris_size = vec2(120.0, 80.0);
const vec2 particle_size = vec2(11.0, 11.0);
const vec2 particle_tex_offset = vec2(120.0, 0.0);

void main() {
  vec2 ratio;

  int i = gl_InstanceIndex;

  if (gl_InstanceIndex == 0) {
    // text
    // I could use offsets and bind the vertex buffer twice
    // but because it's just the text I choose to leave it like this
    if (!isnan(pc.text_size.x) && !isnan(pc.text_size.y)) {
      ratio = pc.text_size / pc.render_dimensions;
      out_tex_coords = tex_coords * pc.text_size;
    } else {
      // make it degenerate
      ratio = vec2(1.0, 1.0);
      out_tex_coords = vec2(0.0, 0.0);
    }
  } else if (gl_InstanceIndex == 1) {
    // ferris
    ratio = ferris_size / pc.render_dimensions;
    out_tex_coords = tex_coords * ferris_size;
  } else {
    // particle
    ratio = particle_size / pc.render_dimensions;
    out_tex_coords = tex_coords * particle_size + particle_tex_offset;
  }

  vec2 final_pos = (vertex_pos * ratio) + ((instance_pos - 0.5) * 2);

  gl_Position = vec4(final_pos.x, final_pos.y, 1.0, 1.0);
}
