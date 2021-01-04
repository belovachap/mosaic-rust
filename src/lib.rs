use image::imageops::resize;
use image::{ImageBuffer, Rgb};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct PicData {
    pub path: PathBuf,
    pub aspect: f64,
    pub thumbnail: ImageBuffer<Rgb<u8>, Vec<u8>>,
}

pub fn get_pic_data(path: PathBuf) -> Option<PicData> {
    match image::open(&path) {
        Ok(img) => {
            let img = img.to_rgb();   
            let aspect = img.width() as f64 / img.height() as f64;
            let thumbnail = resize(&img, 128, 128, image::FilterType::Lanczos3);

            Some(PicData { path: path, aspect: aspect, thumbnail: thumbnail, })   
        },
        Err(_) => None,
    }
}

#[derive(Debug)]
pub struct MatchData {
    pub x: u32,
    pub y: u32,
    pub tile: ImageBuffer<Rgb<u8>, Vec<u8>>,
}

pub fn get_pixel_score(
    thumb1: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    thumb2: &ImageBuffer<Rgb<u8>, Vec<u8>>,
) -> f64 {
    let mut score = 0.0;
    for i in 0..128 {
        for j in 0..128 {
            let p1 = thumb1.get_pixel(i, j);
            let p2 = thumb2.get_pixel(i, j);
            score += (p1.data[0] as f64 - p2.data[0] as f64).abs();
            score += (p1.data[1] as f64 - p2.data[1] as f64).abs();
            score += (p1.data[2] as f64 - p2.data[2] as f64).abs();
        }
    }
    return score / 7500000.0; // normalize the score value a bit
}

pub fn find_best_match(
    aspect: f64,
    thumbnail: &ImageBuffer<Rgb<u8>, Vec<u8>>,
    pics_data: &Vec<PicData>,
) -> PathBuf {
    let mut best_match = None;
    let mut best_score = 100.0;
    for pic_data in pics_data.iter() {
        let aspect_score = (aspect - pic_data.aspect).abs();
        let pixel_score = get_pixel_score(&thumbnail, &pic_data.thumbnail);
        let score = 0.4 * aspect_score + 0.6 * pixel_score;
        if score < best_score {
            best_match = Some(pic_data.path.clone());
            best_score = score;
        }
    }

    best_match.unwrap()
}
