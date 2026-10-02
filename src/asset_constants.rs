pub const NIKO_MODEL_PATH: &str = "./assets/niko.obj";
pub const KAKYOIN_MODEL_PATH: &str = "./assets/Kakyoin.obj";

pub const TEXTURE_PATH: &str = "./assets/textures.ktx";

pub const SPRITES_TOTAL_SIZE: [u32; 2] = [1452, 2048];

pub const FERRIS_TEXTURE_SIZE: [f32; 2] = [428.0, 283.0];
pub const NIKO_TEXTURE_SIZE: [f32; 2] = [1024.0, 1024.0];
pub const KAKYOIN_TEXTURE_SIZE: [f32; 2] = [1024.0, 1024.0];
pub const SOLID_COLOR_TEXTURE_SIZE: [f32; 2] = [1.0, 1.0];

pub const FERRIS_TEXTURE_OFFSET: [f32; 2] = [NIKO_TEXTURE_SIZE[0] + 1.0, 0.0];
pub const NIKO_TEXTURE_OFFSET: [f32; 2] = [0.0, 0.0];
pub const KAKYOIN_TEXTURE_OFFSET: [f32; 2] = [0.0, NIKO_TEXTURE_SIZE[1] + 1.0];
pub const BLACK_TEXTURE_OFFSET: [f32; 2] =
  [NIKO_TEXTURE_SIZE[0] + 1.0, FERRIS_TEXTURE_SIZE[1] + 1.0];
pub const RED_TEXTURE_OFFSET: [f32; 2] = [NIKO_TEXTURE_SIZE[0] + 2.0, FERRIS_TEXTURE_SIZE[1] + 1.0];
pub const GREEN_TEXTURE_OFFSET: [f32; 2] =
  [NIKO_TEXTURE_SIZE[0] + 3.0, FERRIS_TEXTURE_SIZE[1] + 1.0];
pub const BLUE_TEXTURE_OFFSET: [f32; 2] =
  [NIKO_TEXTURE_SIZE[0] + 4.0, FERRIS_TEXTURE_SIZE[1] + 1.0];
