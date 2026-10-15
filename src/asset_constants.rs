pub const NIKO_MODEL_PATH: &str = "./assets/Niko_xinus22.obj";
pub const NIKO_LIGHT_MODEL_PATH: &str = "./assets/NikoLight_xinus22.obj";
pub const KAKYOIN_MODEL_PATH: &str = "./assets/Kakyoin.obj";

pub const TEXTURE_PATH: &str = "./assets/textures.ktx2";

pub const SPRITES_TOTAL_SIZE: [u32; 2] = [1452, 2048];
pub const SPRITES_TOTAL_SIZE_F32: [f32; 2] =
  [SPRITES_TOTAL_SIZE[0] as f32, SPRITES_TOTAL_SIZE[1] as f32];

pub const FERRIS_TEXTURE_SIZE: [f32; 2] = [428.0, 283.0];
pub const NIKO_TEXTURE_SIZE: [f32; 2] = [400.0, 320.0];
pub const KAKYOIN_TEXTURE_SIZE: [f32; 2] = [1024.0, 1024.0];
pub const SOLID_COLOR_TEXTURE_SIZE: [f32; 2] = [1.0, 1.0];

pub const FERRIS_TEXTURE_OFFSET: [f32; 2] = [1024.0 + 1.0, 0.0];
pub const NIKO_TEXTURE_OFFSET: [f32; 2] = [0.0, 0.0];
pub const KAKYOIN_TEXTURE_OFFSET: [f32; 2] = [0.0, 1024.0 + 1.0];
pub const BLACK_TEXTURE_OFFSET: [f32; 2] = [0.0, 0.0];
pub const RED_TEXTURE_OFFSET: [f32; 2] = [1.0, 0.0];
pub const GREEN_TEXTURE_OFFSET: [f32; 2] = [2.0, 0.0];
pub const BLUE_TEXTURE_OFFSET: [f32; 2] = [3.0, 0.0];
