use image::{ImageBuffer, Rgb};
use std::path::PathBuf;

#[derive(Debug)]
pub struct PicData {
    pub path: PathBuf,
    pub aspect: f64,
    pub thumbnail: ImageBuffer<Rgb<u8>, Vec<u8>>,
}
