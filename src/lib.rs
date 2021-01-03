use image::imageops::resize;
use image::{ImageBuffer, Rgb};
use std::path::PathBuf;

#[derive(Debug)]
pub struct PicData {
    pub path: PathBuf,
    pub aspect: f64,
    pub thumbnail: ImageBuffer<Rgb<u8>, Vec<u8>>,
}

pub fn get_pic_data(path: PathBuf) -> PicData {
    let img = image::open(&path).unwrap().to_rgb();
    let aspect = img.width() as f64 / img.height() as f64;
    let thumbnail = resize(&img, 128, 128, image::FilterType::Lanczos3);

    PicData {
        path: path,
        aspect: aspect,
        thumbnail: thumbnail,
    }
}
