use ash::vk;
use cgmath::{EuclideanSpace, Point2};
use harfrust::{script, FontRef, GlyphInfo, Language, ShapeOptions, ShaperData, UnicodeBuffer};
use itertools::Itertools;
use std::{
  collections::HashMap,
  fmt::Debug,
  fs::{self, File, OpenOptions},
  io::{Read, Write},
  mem::offset_of,
  str::FromStr,
};
use ttf_parser::Face;
use vkobjects::utility::any_as_u8_slice;

// Band count is also hardcoded in the shader
const BAND_COUNT: usize = 8;

const LINE_EPSILON: f32 = 0.125;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
/// Quadratic Bezier curve
pub struct QuadCurve {
  pub p0: Point2<f32>,
  pub p1: Point2<f32>,
  pub p2: Point2<f32>,
}

#[derive(Copy, Clone, Debug)]
pub struct Rect {
  pub min: Point2<f32>,
  pub max: Point2<f32>,
}

impl QuadCurve {
  fn line_to_quadratic(a: Point2<f32>, b: Point2<f32>) -> Self {
    let mut mid = Point2 {
      x: (a.x + b.x) / 2.0,
      y: (a.y + b.y) / 2.0,
    };
    let dif = b - a;

    // Perfectly degenerate quadratics interact badly with Slug's root eligibility
    // logic on diagonal segments, causing scanline dropouts. Keep axis-aligned
    // lines exact, but bow diagonal lines by an imperceptible amount so they
    // behave like ordinary quadratics.
    if dif.x.abs() > 0.1 && dif.y.abs() > 0.1 {
      let length = f32::hypot(mid.x, mid.y);
      if length > 0.0 {
        let inv_length = LINE_EPSILON / length;
        mid.x -= dif.y * inv_length;
        mid.y += dif.x * inv_length;
      }
    }

    QuadCurve {
      p0: a,
      p1: mid,
      p2: b,
    }
  }

  fn bounding_box(&self) -> [f32; 4] {
    let [x0, x1, x2] = [self.p0.x, self.p1.x, self.p2.x];
    let [y0, y1, y2] = [self.p0.y, self.p1.y, self.p2.y];

    let cxmin = x0.min(x1).min(x2);
    let cxmax = x0.max(x1).max(x2);
    let cymin = y0.min(y1).min(y2);
    let cymax = y0.max(y1).max(y2);

    [cxmin, cymin, cxmax, cymax]
  }

  pub fn max_x(&self) -> f32 {
    self.p0.x.max(self.p1.x).max(self.p2.x)
  }

  pub fn max_y(&self) -> f32 {
    self.p0.y.max(self.p1.y).max(self.p2.y)
  }
}

#[derive(Clone, Debug)]
pub struct SlugGlyph {
  pub id: u16,
  pub curves: Vec<QuadCurve>,
  pub vertical_bands_curve_indices: [Vec<usize>; BAND_COUNT],
  pub horizontal_bands_curve_indices: [Vec<usize>; BAND_COUNT],
  pub bounding_box: ttf_parser::Rect,
}

/// Extract glyph curves
struct SlugCurveExtractor<'a> {
  pub curves: &'a mut Vec<QuadCurve>,
  pub start: Point2<f32>,
  pub cur_location: Point2<f32>,
}

impl<'a> SlugCurveExtractor<'a> {
  pub fn new(curves: &'a mut Vec<QuadCurve>) -> Self {
    Self {
      curves,
      start: Point2 { x: 0.0, y: 0.0 },
      cur_location: Point2 { x: 0.0, y: 0.0 },
    }
  }
}

// see ttf_parser::OutlineBuilder
impl<'a> ttf_parser::OutlineBuilder for SlugCurveExtractor<'a> {
  fn move_to(&mut self, x: f32, y: f32) {
    self.start = Point2 { x, y };
    self.cur_location = self.start;
  }

  fn line_to(&mut self, x: f32, y: f32) {
    let to = Point2 { x, y };
    let diff = to - self.cur_location;
    // ignore vertical / horizontal lines
    if diff.x.abs() > 0.1 || diff.y.abs() > 0.1 {
      self
        .curves
        .push(QuadCurve::line_to_quadratic(self.cur_location, to));
    }
    self.cur_location = to;
  }

  fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
    let to = Point2 { x: x, y: y };
    self.curves.push(QuadCurve {
      p0: self.cur_location,
      p1: Point2 { x: x1, y: y1 },
      p2: to,
    });
    self.cur_location = to;
  }

  fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
    let p0 = self.cur_location;
    let p1 = Point2 { x: x1, y: y1 };
    let p2 = Point2 { x: x2, y: y2 };
    let p3 = Point2 { x, y };

    let m01 = p0.midpoint(p1);
    let m12 = p1.midpoint(p2);
    let m23 = p2.midpoint(p3);
    let m012 = m01.midpoint(m12);
    let m123 = m12.midpoint(m23);
    let mid = m012.midpoint(m123);

    self.curves.push(QuadCurve {
      p0,
      p1: m01,
      p2: mid,
    });
    self.curves.push(QuadCurve {
      p0: mid,
      p1: m123,
      p2: p3,
    });

    self.cur_location = p3;
  }

  fn close(&mut self) {
    let full_vec = self.start - self.cur_location;
    // ignore vertical / horizontal lines
    if full_vec.x.abs() > 0.1 || full_vec.y.abs() > 0.1 {
      self
        .curves
        .push(QuadCurve::line_to_quadratic(self.cur_location, self.start));
    }

    self.cur_location = self.start;
  }
}

fn build_glyph_bands(
  curves: &[QuadCurve],
  bounding_box: Rect,
) -> ([Vec<usize>; BAND_COUNT], [Vec<usize>; BAND_COUNT]) {
  let Rect { min, max } = bounding_box;
  let width = max.x - min.x;
  let height = max.y - min.y;

  let mut hbands: [Vec<usize>; BAND_COUNT] = Default::default();
  let mut vbands: [Vec<usize>; BAND_COUNT] = Default::default();

  for (c_i, curve) in curves.iter().enumerate() {
    let [cxmin, cymin, cxmax, cymax] = curve.bounding_box();

    println!(
      "{} {:?} {} {} {} {}",
      c_i, curve, cxmin, cymin, cxmax, cymax
    );

    // horizontal bands
    {
      let b0 = (((cymin - min.y) / height) * BAND_COUNT as f32) as usize;
      let b1 = (((cymax - min.y) / height) * BAND_COUNT as f32) as usize;
      for b in b0..=(b1.min(BAND_COUNT - 1)) {
        hbands[b].push(c_i);
      }
    }

    // 0 QuadCurve { p0: Point2 [0.0, -25.0], p1: Point2 [212.2517, 733.0696], p2: Point2 [425.0, 1491.0] } 0 -25 425 1491
    // horizontal 0 8
    // vertical 0 5

    // vertical bands
    {
      let b0 = ((cxmin - min.x) / width * BAND_COUNT as f32) as usize;
      let b1 = ((cxmax - min.x) / width * BAND_COUNT as f32) as usize;
      for b in b0..=(b1.min(BAND_COUNT - 1)) {
        vbands[b].push(c_i);
      }
    }
  }

  return (hbands, vbands);
}

pub const TEX_WIDTH: usize = 4096;

#[derive(Clone, Debug)]
struct PackedGlyphData {
  curve_tex_data: Vec<[f32; 4]>,
  band_tex_data: Vec<u32>,
  curve_tex_height: usize,
  band_tex_height: usize,
  glyph_band_info: Vec<(usize, usize)>,
  glyph_curve_starts: Vec<usize>,
}

fn pack_glyph_data(glyphs: &mut [SlugGlyph]) -> PackedGlyphData {
  // --- Curve texture (RGBA32Float, width 4096) ---
  // Each curve = 2 texels: (p0x, p0y, p1x, p1y) and (p2x, p2y, 0, 0)
  let mut total_curve_texels = 0;
  for g in glyphs.iter() {
    total_curve_texels += g.curves.len() * 2;
  }

  let curve_tex_height = (total_curve_texels / TEX_WIDTH) + 1;
  let mut curve_tex_data = vec![[0f32; 4]; TEX_WIDTH * curve_tex_height];

  let mut curve_texel_idx = 0;
  let mut glyph_curve_starts = Vec::new();

  for g in glyphs.iter() {
    glyph_curve_starts.push(curve_texel_idx);
    for c in g.curves.iter() {
      // Texel 0: (p0x, p0y, p1x, p1y)
      let i0 = curve_texel_idx;
      curve_tex_data[i0] = [c.p0.x, c.p0.y, c.p1.x, c.p1.y];

      // Texel 1: (p2x, p2y, 0, 0)
      let i1 = curve_texel_idx + 1;
      curve_tex_data[i1][0] = c.p2.x;
      curve_tex_data[i1][1] = c.p2.y;

      curve_texel_idx += 2;
    }
  }

  // --- Band texture (RGBA32Uint, width 4096) ---
  // Per glyph: [hBand headers...] [vBand headers...] [curve index lists...]
  // Each header texel: (curveCount, offsetFromGlyphLoc, 0, 0)
  // Each curve ref texel: (curveTexX, curveTexY, 0, 0)
  let mut total_band_texels = 0;
  for g in glyphs.iter() {
    let header_count =
      g.horizontal_bands_curve_indices.len() + g.vertical_bands_curve_indices.len();
    // Pad to avoid header wrapping at row boundary
    let padded = TEX_WIDTH - (total_band_texels % TEX_WIDTH);
    if padded < header_count && padded < TEX_WIDTH {
      total_band_texels += padded;
    }
    total_band_texels += header_count;
    for indices in g
      .horizontal_bands_curve_indices
      .iter()
      .chain(g.vertical_bands_curve_indices.iter())
    {
      total_band_texels += indices.len();
    }
  }

  let band_tex_height = (total_band_texels / TEX_WIDTH) + 1;
  let mut band_tex_data = vec![0u32; TEX_WIDTH * band_tex_height * 4];

  let mut band_texel_idx = 0;
  let mut glyph_band_info = Vec::new();

  for (gi, g) in glyphs.iter_mut().enumerate() {
    let hband_count = g.horizontal_bands_curve_indices.len();
    let vband_count = g.vertical_bands_curve_indices.len();
    let header_count = hband_count + vband_count;

    // Ensure headers don't straddle a row boundary
    let cur_x = band_texel_idx % TEX_WIDTH;
    if cur_x + header_count > TEX_WIDTH {
      band_texel_idx = ((band_texel_idx / TEX_WIDTH) + 1) * TEX_WIDTH;
    }

    let glyph_loc_x = band_texel_idx % TEX_WIDTH;
    let glyph_loc_y = band_texel_idx / TEX_WIDTH;
    glyph_band_info.push((glyph_loc_x, glyph_loc_y));

    let glyph_start = band_texel_idx;
    let glyph_curve_start = glyph_curve_starts[gi];

    // Sort curves: h-bands by descending max x, v-bands by descending max y
    for curve_indices in g.horizontal_bands_curve_indices.iter_mut() {
      curve_indices.sort_by(|&a, &b| {
        let curve1_max_x = g.curves[a].max_x();
        let curve2_max_x = g.curves[b].max_x();
        // reverse ordering
        curve2_max_x.total_cmp(&curve1_max_x)
      });
    }
    for curve_indices in g.vertical_bands_curve_indices.iter_mut() {
      curve_indices.sort_by(|&a, &b| {
        let curve1_max_y = g.curves[a].max_y();
        let curve2_max_y = g.curves[b].max_y();
        // reverse ordering
        curve2_max_y.total_cmp(&curve1_max_y)
      });
    }

    // Calculate offsets: curve lists follow all headers
    let mut curve_list_offset = header_count;
    let mut band_offsets = Vec::new();
    for curve_indices in g
      .horizontal_bands_curve_indices
      .iter()
      .chain(g.vertical_bands_curve_indices.iter())
    {
      band_offsets.push(curve_list_offset);
      curve_list_offset += curve_indices.len();
    }

    // Write band headers
    for (i, band_indices) in g
      .horizontal_bands_curve_indices
      .iter()
      .chain(g.vertical_bands_curve_indices.iter())
      .enumerate()
    {
      let tl = glyph_start + i;
      let di = tl * 4;
      band_tex_data[di] = band_indices.len() as u32;
      band_tex_data[di + 1] = band_offsets[i] as u32;

      let list_start = glyph_start + band_offsets[i];
      for (j, curve_i) in band_indices.iter().enumerate() {
        let curve_texel = glyph_curve_start + curve_i * 2;
        let curve_tex_x = curve_texel % TEX_WIDTH;
        let curve_tex_y = curve_texel / TEX_WIDTH;

        let tl = list_start + j;
        let di = tl * 4;
        band_tex_data[di] = curve_tex_x as u32;
        band_tex_data[di + 1] = curve_tex_y as u32;
      }
    }

    band_texel_idx = glyph_start + curve_list_offset;
  }

  let text = band_tex_data[0..300].iter().join("");
  println!("band_tex_data {:?}", &text);

  PackedGlyphData {
    curve_tex_data,
    band_tex_data,
    curve_tex_height,
    band_tex_height,
    glyph_band_info,
    glyph_curve_starts,
  }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlugVertexGlyphInBandLocation {
  pub x: u16,
  pub y: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlugVertexMaxBandIndices {
  pub max_band_x: u16,
  pub max_band_y: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SlugBertexBandInfo {
  pub scale_x: f32,
  pub scale_y: f32,
  pub offset_x: f32,
  pub offset_y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SlugVertex {
  // pos
  pub obj_space_vertex_coords: [f32; 2],
  pub obj_space_normal_vector: [f32; 2],

  // tex
  pub em_space_sample_coords: [f32; 2],
  pub glyph_in_band_loc: SlugVertexGlyphInBandLocation,
  pub max_band_indices: SlugVertexMaxBandIndices,

  // jac
  pub jac: [f32; 4],
  // bnd
  pub band: SlugBertexBandInfo,
  // col
  pub color: [f32; 4],
}

impl SlugVertex {
  const ATTRIBUTE_SIZE: usize = 5;

  pub const fn get_binding_description(binding: u32) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription {
      binding,
      stride: size_of::<Self>() as u32,
      input_rate: vk::VertexInputRate::VERTEX,
    }
  }

  pub const fn get_attribute_descriptions(
    offset: u32,
    binding: u32,
  ) -> [vk::VertexInputAttributeDescription; Self::ATTRIBUTE_SIZE] {
    [
      vk::VertexInputAttributeDescription {
        location: offset,
        binding,
        format: vk::Format::R32G32B32A32_SFLOAT,
        offset: offset_of!(Self, obj_space_vertex_coords) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 1,
        binding,
        format: vk::Format::R32G32B32A32_SFLOAT,
        offset: offset_of!(Self, em_space_sample_coords) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 2,
        binding,
        format: vk::Format::R32G32B32A32_SFLOAT,
        offset: offset_of!(Self, jac) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 3,
        binding,
        format: vk::Format::R32G32B32A32_SFLOAT,
        offset: offset_of!(Self, band) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 4,
        binding,
        format: vk::Format::R32G32B32A32_SFLOAT,
        offset: offset_of!(Self, color) as u32,
      },
    ]
  }
}

#[derive(Clone, Debug)]
pub struct PrepareTextResult {
  pub glyphs: Vec<SlugGlyph>,
  pub vertices: Vec<SlugVertex>,
  pub indices: Vec<u32>,
  pub curve_tex_data: Vec<[f32; 4]>,
  pub band_tex_data: Vec<u32>,
  pub curve_tex_height: usize,
  pub band_tex_height: usize,
}

#[allow(dead_code)]
impl PrepareTextResult {
  pub fn save_to_file(&self) {
    let vertices_slice = unsafe {
      let bytes = self.vertices.len() * size_of::<SlugVertex>();
      std::slice::from_raw_parts(self.vertices.as_ptr() as *const u8, bytes)
    };
    let indices_slice = unsafe {
      let bytes = self.indices.len() * size_of::<u32>();
      std::slice::from_raw_parts(self.indices.as_ptr() as *const u8, bytes)
    };
    let curve_tex_slice = unsafe {
      let bytes = self.curve_tex_data.len() * size_of::<[f32; 4]>();
      std::slice::from_raw_parts(self.curve_tex_data.as_ptr() as *const u8, bytes)
    };
    let band_tex_slice = unsafe {
      let bytes = self.band_tex_data.len() * size_of::<u32>();
      std::slice::from_raw_parts(self.band_tex_data.as_ptr() as *const u8, bytes)
    };

    let mut vertices = File::create("./vertices").unwrap();
    vertices.write_all(&vertices_slice).unwrap();
    vertices.flush().unwrap();

    let mut indices = File::create("./indices").unwrap();
    indices.write_all(&indices_slice).unwrap();
    indices.flush().unwrap();

    let mut curve_tex = File::create("./curve_tex").unwrap();
    curve_tex.write_all(&curve_tex_slice).unwrap();
    curve_tex.flush().unwrap();

    let mut band_tex = File::create("./band_tex").unwrap();
    band_tex.write_all(&band_tex_slice).unwrap();
    band_tex.flush().unwrap();
  }

  pub fn save_to_file_text(&self) {
    let mut vertices = OpenOptions::new()
      .append(true)
      .open("./testing/vertices.txt")
      .unwrap();
    vertices
      .write_fmt(format_args!("{:?}\n", self.vertices))
      .unwrap();
    vertices.flush().unwrap();

    let mut indices = OpenOptions::new()
      .append(true)
      .open("./testing/indices.txt")
      .unwrap();
    indices
      .write_fmt(format_args!("{:?}\n", self.indices))
      .unwrap();
    indices.flush().unwrap();

    let mut curve_tex = OpenOptions::new()
      .append(true)
      .open("./testing/curve_tex.txt")
      .unwrap();
    curve_tex
      .write_fmt(format_args!("{:?}\n", self.curve_tex_data))
      .unwrap();
    curve_tex.flush().unwrap();

    let mut band_tex = OpenOptions::new()
      .append(true)
      .open("./testing/band_tex.txt")
      .unwrap();
    band_tex
      .write_fmt(format_args!("{:?}\n", self.band_tex_data))
      .unwrap();
    band_tex.flush().unwrap();
  }

  pub fn compare_to_file(&self) {
    let mut vertices_file = File::open("./vertices").unwrap();
    let mut vertices_bytes = Vec::new();
    vertices_file.read_to_end(&mut vertices_bytes).unwrap();

    let mut indices_file = File::open("./indices").unwrap();
    let mut indices_bytes = Vec::new();
    indices_file.read_to_end(&mut indices_bytes).unwrap();

    let mut curve_tex_file = File::open("./curve_tex").unwrap();
    let mut curve_tex_bytes = Vec::new();
    curve_tex_file.read_to_end(&mut curve_tex_bytes).unwrap();

    let mut band_tex_file = File::open("./band_tex").unwrap();
    let mut band_tex_bytes = Vec::new();
    band_tex_file.read_to_end(&mut band_tex_bytes).unwrap();

    let vertices_slice = unsafe {
      let bytes = vertices_bytes.len() / size_of::<SlugVertex>();
      std::slice::from_raw_parts(vertices_bytes.as_ptr() as *const SlugVertex, bytes)
    };
    let indices_slice = unsafe {
      let bytes = indices_bytes.len() / size_of::<u32>();
      std::slice::from_raw_parts(indices_bytes.as_ptr() as *const u32, bytes)
    };
    let curve_tex_slice = unsafe {
      let bytes = curve_tex_bytes.len() / size_of::<[f32; 4]>();
      std::slice::from_raw_parts(curve_tex_bytes.as_ptr() as *const [f32; 4], bytes)
    };
    let band_tex_slice = unsafe {
      let bytes = band_tex_bytes.len() / size_of::<u32>();
      std::slice::from_raw_parts(band_tex_bytes.as_ptr() as *const u32, bytes)
    };

    let mut wrong_vertices = 0usize;
    for (&a, &b) in vertices_slice.iter().zip(self.vertices.iter()) {
      unsafe {
        if any_as_u8_slice(&a) != any_as_u8_slice(&b) {
          wrong_vertices += 1;
        }
      }
    }

    log::error!("vertices equal count: {}", wrong_vertices);
    log::error!("indices equal: {}", indices_slice == self.indices);
    log::error!(
      "curve_tex equal: {}",
      curve_tex_slice == self.curve_tex_data
    );
    log::error!("band_tex equal: {}", band_tex_slice == self.band_tex_data);
  }
}

pub fn prepare_text(text: &str, font_size: usize) -> PrepareTextResult {
  let mut buffer = UnicodeBuffer::new();
  buffer.push_str(text);

  buffer.set_direction(harfrust::Direction::LeftToRight);
  buffer.set_script(script::LATIN);
  buffer.set_language(Language::from_str("en").unwrap());

  let font_bytes = std::fs::read("c:\\windows\\Fonts\\arial.ttf").unwrap();
  let font = FontRef::new(&font_bytes).expect("Failed to read font data");

  let shaper_data = ShaperData::new(&font);
  let shaper = shaper_data.shaper(&font).build();

  let glyph_buffer = shaper.shape(buffer, ShapeOptions::new());
  let scale = font_size as f32 / (shaper.units_per_em() as f32);

  let face = Face::parse(&font_bytes, 0).unwrap();
  println!("Font face name: {:?}", face.names());

  let mut glyph_map: HashMap<u16, SlugGlyph> = HashMap::new();
  for glyph_info in glyph_buffer.glyph_infos() {
    let glyph_id = glyph_info.glyph_id.try_into().unwrap();
    if glyph_map.contains_key(&glyph_id) {
      continue;
    }

    let mut curves = Vec::new();
    let mut curve_extractor = SlugCurveExtractor::new(&mut curves);

    // extracts curves here
    let int_bbox = match face.outline_glyph(ttf_parser::GlyphId(glyph_id), &mut curve_extractor) {
      Some(outline) => outline,
      None => {
        continue;
      }
    };

    let bounding_box = Rect {
      min: Point2 {
        x: int_bbox.x_min as f32,
        y: int_bbox.y_min as f32,
      },
      max: Point2 {
        x: int_bbox.x_max as f32,
        y: int_bbox.y_max as f32,
      },
    };

    let (hbands, vbands) = build_glyph_bands(&curves, bounding_box);
    glyph_map.insert(
      glyph_id,
      SlugGlyph {
        id: glyph_id,
        curves: curves,
        vertical_bands_curve_indices: vbands,
        horizontal_bands_curve_indices: hbands,
        bounding_box: int_bbox,
      },
    );
  }

  let mut glyphs: Vec<SlugGlyph> = glyph_map.into_values().collect();
  // sort glyphs for testing
  // glyphs.sort_by_key(|k| k.id);

  let packed = pack_glyph_data(&mut glyphs);

  let mut glyph_data_map = HashMap::new();
  for (i, glyph) in glyphs.iter().enumerate() {
    let (glyph_loc_x, glyph_loc_y) = packed.glyph_band_info[i];
    glyph_data_map.insert(glyph.id, (glyph, glyph_loc_x, glyph_loc_y));
  }

  let mut vertices = Vec::new();
  let mut indices = Vec::new();
  let mut cursor_x = 0;
  let mut cursor_y = 0;
  let mut quad_idx: u32 = 0;
  for (info, pos) in glyph_buffer
    .glyph_infos()
    .iter()
    .zip(glyph_buffer.glyph_positions().iter())
  {
    let glyph_id = info.glyph_id as u16;
    let (glyph, glyph_loc_x, glyph_loc_y) = match glyph_data_map.get(&glyph_id) {
      Some(values) => values,
      None => {
        // empty glyph -> skip
        cursor_x += pos.x_advance;
        cursor_y += pos.y_advance;
        continue;
      }
    };
    let bbox = glyph.bounding_box;

    let width = bbox.x_max - bbox.x_min;
    let height = bbox.y_max - bbox.y_min;

    // Object-space position (Y-up screen pixels)
    let ox = (cursor_x + pos.x_offset) as f32 * scale;
    let oy = (cursor_y + pos.y_offset) as f32 * scale;
    let x0 = ox + bbox.x_min as f32 * scale;
    let y0 = oy + bbox.y_min as f32 * scale;
    let x1 = ox + bbox.x_max as f32 * scale;
    let y1 = oy + bbox.y_max as f32 * scale;

    // Band transform: maps em-space to band indices
    let band_scale_x = if width > 0 {
      glyph.vertical_bands_curve_indices.len() as f32 / width as f32
    } else {
      0.0
    };
    let band_scale_y = if height > 0 {
      glyph.horizontal_bands_curve_indices.len() as f32 / height as f32
    } else {
      0.0
    };
    let band_offset_x = -bbox.x_min as f32 * band_scale_x;
    let band_offset_y = -bbox.y_min as f32 * band_scale_y;

    let band_max_x = glyph.vertical_bands_curve_indices.len() - 1;
    let band_max_y = glyph.horizontal_bands_curve_indices.len() - 1;

    let inv_scale = 1.0 / scale;

    let corners = [
      [x0, -y0, -1.0, -1.0, bbox.x_min as f32, bbox.y_min as f32], // bottom-left
      [x1, -y0, 1.0, -1.0, bbox.x_max as f32, bbox.y_min as f32],  // bottom-right
      [x1, -y1, 1.0, 1.0, bbox.x_max as f32, bbox.y_max as f32],   // top-right
      [x0, -y1, -1.0, 1.0, bbox.x_min as f32, bbox.y_max as f32],  // top-left
    ];
    for [px, py, nx, ny, ex, ey] in corners {
      let vertex = SlugVertex {
        // pos (location 0): object-space position + normal
        obj_space_vertex_coords: [px, py],
        obj_space_normal_vector: [nx, ny],

        // tex (location 1): em-space coords + packed glyph/band data
        em_space_sample_coords: [ex, ey],
        glyph_in_band_loc: SlugVertexGlyphInBandLocation {
          x: (*glyph_loc_x).try_into().unwrap(),
          y: (*glyph_loc_y).try_into().unwrap(),
        },
        max_band_indices: SlugVertexMaxBandIndices {
          max_band_x: band_max_x.try_into().unwrap(),
          max_band_y: band_max_y.try_into().unwrap(),
        },

        // jac (location 2): inverse Jacobian (d(em)/d(obj))
        jac: [inv_scale, 0.0, 0.0, inv_scale],
        // bnd (location 3): band transform (scale + offset)
        band: SlugBertexBandInfo {
          scale_x: band_scale_x,
          scale_y: band_scale_y,
          offset_x: band_offset_x,
          offset_y: band_offset_y,
        },
        color: [0.0, 1.0, 0.0, 1.0],
      };
      vertices.push(vertex);
    }

    let base = quad_idx * 4;
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    cursor_x += pos.x_advance;
    cursor_y += pos.y_advance;
    quad_idx += 1;
  }

  PrepareTextResult {
    glyphs,
    vertices,
    indices,
    curve_tex_data: packed.curve_tex_data,
    band_tex_data: packed.band_tex_data,
    curve_tex_height: packed.curve_tex_height,
    band_tex_height: packed.band_tex_height,
  }
}
