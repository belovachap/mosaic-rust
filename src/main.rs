extern crate image;

use rand::distributions::{Distribution, Uniform};

use image::imageops::{crop, replace, resize};
use image::{ImageBuffer, Rgb};

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::iter::Enumerate;

use itertools::Itertools;

use rayon::prelude::*;

#[derive(Debug)]
struct PicData {
    path: PathBuf,
    aspect: f64,
    thumbnail: ImageBuffer<Rgb<u8>, Vec<u8>>,
}

#[derive(Debug)]
struct MatchData {
    x: u32,
    y: u32,
    tile: ImageBuffer<Rgb<u8>, Vec<u8>>,
}

fn get_pixel_score(thumb1: & ImageBuffer<Rgb<u8>, Vec<u8>>, thumb2: & ImageBuffer<Rgb<u8>, Vec<u8>>) -> f64 {
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

fn find_best_match(aspect: f64, thumbnail: & ImageBuffer<Rgb<u8>, Vec<u8>>, pics_data: & Vec<PicData>) -> PathBuf {
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

    return best_match.unwrap();
}


fn get_pic_data(path: PathBuf) -> PicData {
    let img = image::open(&path).unwrap().to_rgb();
    let aspect = img.width() as f64 / img.height() as f64;
    let thumbnail = resize(&img, 128, 128, image::FilterType::Lanczos3);
    
    PicData{path: path, aspect: aspect, thumbnail: thumbnail}
}

fn get_pics_data(pics_dir: &Path) -> Vec<PicData> {
    let paths = fs::read_dir(pics_dir).unwrap().map(|x| x.unwrap().path());
 
    paths.par_bridge().map(get_pic_data).collect()
}

fn get_match_data(
    google_img: & ImageBuffer<Rgb<u8>, Vec<u8>>,
    pics_data: & Vec<PicData>,
    x_rulers: & Vec<u32>,
    y_rulers: & Vec<u32>,
    x_steps: Enumerate<std::slice::Iter<'_, u32>>,
    y_steps: Enumerate<std::slice::Iter<'_, u32>>,
) -> Vec<MatchData> {
    x_steps.cartesian_product(y_steps).par_bridge().map(|((i, &x), (j, &y))| {
        let width = x_rulers[i+1] - x;
        let height = y_rulers[j+1] - y;
        println!("{}, {}, {}, {}, {}, {}", i, j, x, y, width, height);
            
        let mut crop_img = google_img.clone();
        let crop = crop(&mut crop_img, x, y, x + width, y + height).to_image();
        let aspect = width as f64 / height as f64;
        let thumbnail = resize(&crop, 128, 128, image::FilterType::Lanczos3);

        let best_match = find_best_match(aspect, &thumbnail, &pics_data);
        let best_image = image::open(best_match).unwrap().to_rgb();
        let best_resize = resize(&best_image, width, height, image::FilterType::Lanczos3);

        MatchData{ x: x, y: y, tile: best_resize }
    }).collect()
}

fn main() {
    let mut base_dir = env::current_dir().unwrap();
    base_dir.push("resources");

    let mut reddit_pics_dir = base_dir.clone();
    reddit_pics_dir.push("top_reddit_pics/");

    let mut google_pic_dir = base_dir.clone();
    google_pic_dir.push("top_google_pic/");

    let mut mosaic_dir = base_dir.clone();
    mosaic_dir.push("mosaic/");

    let google_img_name = fs::read_dir(google_pic_dir).unwrap().next().unwrap().unwrap();
    let mut google_img = image::open(google_img_name.path()).unwrap().to_rgb();

    let mut x_rulers = Vec::new();

    let mut pixels = 0;
    let distribution = Uniform::new(120, 320);
    let mut rng = rand::thread_rng();
    loop {
        let step = distribution.sample(&mut rng);

        if pixels + step > google_img.width() {
            break;
        }

        pixels += step;
        x_rulers.push(pixels);
    }

    let mut x_remaining = google_img.width() - pixels;
    while x_remaining > 0 {
        for i in 0..x_rulers.len() {
            for j in i..x_rulers.len() {
                x_rulers[j] += 1;
            }
            
            x_remaining -= 1;
            if x_remaining <= 0 {
                break;
            }
        }
    }

    x_rulers.insert(0, 0);

    let mut y_rulers = Vec::new();

    let mut pixels = 0;
    loop {
        let step = distribution.sample(&mut rng);

        if pixels + step > google_img.height() {
            break;
        }

        pixels += step;
        y_rulers.push(pixels);
    }

    let mut y_remaining = google_img.height() - pixels;
    while y_remaining > 0 {
        for i in 0..y_rulers.len() {
            for j in i..y_rulers.len() {
                y_rulers[j] += 1;
            }
            
            y_remaining -= 1;
            if y_remaining <= 0 {
                break;
            }
        }
    }

    y_rulers.insert(0, 0);

    let pics_data = get_pics_data(&reddit_pics_dir);
    let x_steps = x_rulers[0..x_rulers.len()-1].iter().enumerate();
    let y_steps = y_rulers[0..y_rulers.len()-1].iter().enumerate();
    let match_data = get_match_data(&google_img, &pics_data, &x_rulers, &y_rulers, x_steps, y_steps);

    for m in match_data.iter() {
        replace(&mut google_img, &m.tile, m.x, m.y);
    }

    mosaic_dir.push(google_img_name.file_name());
    image::save_buffer(
        &mosaic_dir,
        &google_img,
        google_img.width(),
        google_img.height(),
        image::ColorType::RGB(8)
    ).unwrap();      
}
