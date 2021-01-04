use std::cell::RefCell;
use std::env::args;
use std::fs;
use std::iter::Enumerate;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

use gio::prelude::*;
use glib::clone;
use gtk::prelude::*;
use image::imageops::{crop, replace, resize};
use image::{ImageBuffer, Rgb};
use itertools::Itertools;
use rand::distributions::{Distribution, Uniform};
use rayon::prelude::*;

use mlib::*;

pub fn main() {
    glib::set_program_name(Some("Photo Mosaic"));

    let application = gtk::Application::new(
        Some("com.chapmanshoop.photo-mosaic"),
        gio::ApplicationFlags::empty(),
    )
    .expect("initialization failed");

    application.connect_startup(|app| {
        let application = Application::new(app);

        let application_container = RefCell::new(Some(application));
        app.connect_shutdown(move |_| {
            let application = application_container
                .borrow_mut()
                .take()
                .expect("Shutdown called multiple times");
            // Here we could do whatever we need to do for shutdown now
            drop(application);
        });
    });

    application.connect_activate(|_| {});
    application.run(&args().collect::<Vec<_>>());
}

pub struct Application {
    pub widgets: Rc<Widgets>,
}

impl Application {
    pub fn new(app: &gtk::Application) -> Self {
        let app = Application {
            widgets: Rc::new(Widgets::new(app)),
        };

        app
    }
}

pub struct Widgets {
    pub window: gtk::ApplicationWindow,
    pub header: Header,
    pub main_view: MainView,
}

impl Widgets {
    pub fn new(application: &gtk::Application) -> Self {
        let header = Header::new();
        let window = gtk::ApplicationWindow::new(application);
        let main_view = MainView::new(&window);
        window.set_icon_name(Some("package-x-generic"));
        window.set_property_window_position(gtk::WindowPosition::Center);
        window.set_titlebar(Some(&header.container));
        window.add(&main_view.container);
        window.show_all();
        window.set_default_size(500, 250);
        window.connect_delete_event(move |window, _| {
            window.close();
            Inhibit(false)
        });

        Widgets {
            window,
            header,
            main_view,
        }
    }
}

pub struct Header {
    container: gtk::HeaderBar,
}

impl Header {
    pub fn new() -> Self {
        let container = gtk::HeaderBar::new();
        container.set_title(Some("Photo Mosaic"));
        container.set_show_close_button(true);

        Header { container }
    }
}

pub struct MainView {
    pub container: gtk::Grid,

    pub pics_data: Arc<Mutex<Vec<PicData>>>,
    pub pics_data_chooser_button: gtk::FileChooserButton,
    pub pics_data_progress: gtk::ProgressBar,

    pub input: Arc<Mutex<Option<ImageBuffer<Rgb<u8>, Vec<u8>>>>>,
    pub input_chooser_button: gtk::FileChooserButton,
    pub input_progress: gtk::ProgressBar,

    pub output_chooser_button: gtk::Button,
    pub match_data_progress: gtk::ProgressBar,
}

impl MainView {
    pub fn new(window: &gtk::ApplicationWindow) -> Self {
        let pics_data = Arc::new(Mutex::new(Vec::new()));

        let pics_data_progress = gtk::ProgressBar::new();
        pics_data_progress.set_text(Some("0 Pictures Loaded"));
        pics_data_progress.set_show_text(true);
        pics_data_progress.set_hexpand(true);

        let pics_data_chooser_button = gtk::FileChooserButton::new("Select Picture", gtk::FileChooserAction::SelectFolder);
        pics_data_chooser_button.connect_file_set(
            clone!(@weak pics_data, @weak pics_data_progress, @weak window => move |button| {
                let path = button.get_filename().expect("Couldn't get filename");
                let total_files = fs::read_dir(&path).unwrap().count();
                let paths = fs::read_dir(&path).unwrap().map(|x| x.unwrap().path());
                let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
                let local_pics_data = Arc::new(Mutex::new(Vec::new()));
                
                thread::spawn(clone!(@weak local_pics_data => move || {
                    paths.par_bridge().for_each(|path| {
                        match get_pic_data(path) {
                            Some(pic_data) => {
                                local_pics_data.lock().unwrap().push(pic_data);
                                tx.send(Some(1)).unwrap(); 
                            },
                            None => tx.send(Some(0)).unwrap(),
                        }
                    });                        
                    tx.send(None).unwrap();
                }));

                let mut num_processed = 0;
                let mut num_loaded = 0;
                rx.attach(None, move |value| match value {
                    Some(value) => {
                        num_processed += 1;
                        num_loaded += value;
                        
                        pics_data_progress.set_text(Some(&(num_loaded.to_string() + " Pictures Loaded")));
                        pics_data_progress.set_fraction(num_processed as f64 / total_files as f64);

                        glib::Continue(true)
                    }
                    None => {
                        *(pics_data.lock().unwrap()) = local_pics_data.lock().unwrap().to_vec();

                        glib::Continue(false)
                    }
                });
            })
        );

        let input: Arc<Mutex<Option<ImageBuffer<Rgb<u8>, Vec<u8>>>>> = Arc::new(Mutex::new(None));

        let input_progress = gtk::ProgressBar::new();
        input_progress.set_text(Some("No Photo Selected"));
        input_progress.set_show_text(true);
        input_progress.set_hexpand(true);

        let input_chooser_button =
            gtk::FileChooserButton::new("Select Picture", gtk::FileChooserAction::Open);
        input_chooser_button.connect_file_set(
            clone!(@weak input, @weak input_progress => move |button| {
                let path = button.get_filename().unwrap();
                println!("You selected: {:?}", path);
                *input.lock().unwrap() = Some(image::open(path).unwrap().to_rgb());
            }),
        );

        let match_data_progress = gtk::ProgressBar::new();
        match_data_progress.set_text(Some("0 Tiles Placed"));
        match_data_progress.set_show_text(true);
        match_data_progress.set_hexpand(true);

        let output_chooser_button = gtk::Button::with_label("Create Photo Mosaic");
        output_chooser_button.connect_clicked(clone!(@weak input, @weak pics_data, @weak match_data_progress, @weak window => move |button| {
            let pics_dataz = pics_data.lock().unwrap();
            println!("I unwrapped pics_data, it has {} elements", pics_dataz.len());
            let file_chooser = gtk::FileChooserDialog::new(
                Some("Create Photo Mosaic"),
                Some(&window),
                gtk::FileChooserAction::Save
            ); 
            file_chooser.add_buttons(&[
                ("Create", gtk::ResponseType::Ok),
                ("Cancel", gtk::ResponseType::Cancel),
            ]);
            file_chooser.connect_response(clone!(@weak input, @weak pics_data, @weak match_data_progress => move |file_chooser, response| {
                if response == gtk::ResponseType::Ok {
                    let input_data = input.lock().unwrap().as_ref().clone().unwrap().clone();
                    let path = file_chooser.get_filename().expect("Couldn't get filename");
                    println!("You selected: {:?}", path);
                    println!("Create the output!");

                        let mut x_rulers = Vec::new();

                        let mut pixels = 0;
                        let distribution = Uniform::new(120, 320);
                        let mut rng = rand::thread_rng();
                        loop {
                            let step = distribution.sample(&mut rng);

                            if pixels + step > input_data.width() {
                                break;
                            }

                            pixels += step;
                            x_rulers.push(pixels);
                        }

                        let mut x_remaining = input_data.width() - pixels;
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

                            if pixels + step > input_data.height() {
                                break;
                            }

                            pixels += step;
                            y_rulers.push(pixels);
                        }

                        let mut y_remaining = input_data.height() - pixels;
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

                    let total_tiles = (x_rulers.len() - 1)  * (y_rulers.len() - 1);
                    println!("Total tiles: {:?}", total_tiles);

                    let local_match_data = Arc::new(Mutex::new(Vec::new()));
                    let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

                    thread::spawn(clone!(@weak local_match_data, @weak input => move || {
                        let pics_data = pics_data.lock().unwrap();

                        let x_steps = x_rulers[0..x_rulers.len() - 1].iter().enumerate();
                        let y_steps = y_rulers[0..y_rulers.len() - 1].iter().enumerate();
                        x_steps
                            .cartesian_product(y_steps)
                            .par_bridge()
                            .for_each(|((i, &x), (j, &y))| {
                                let width = x_rulers[i + 1] - x;
                                let height = y_rulers[j + 1] - y;
                                println!("{}, {}, {}, {}, {}, {}", i, j, x, y, width, height);

                                let mut crop_img = input.lock().unwrap().as_ref().clone().unwrap().clone();
                                let crop = crop(&mut crop_img, x, y, width, height).to_image();
                                let aspect = width as f64 / height as f64;
                                let thumbnail = resize(&crop, 128, 128, image::FilterType::Lanczos3);

                                let best_match = find_best_match(aspect, &thumbnail, &pics_data);
                                let best_image = image::open(best_match).unwrap().to_rgb();
                                let best_resize = resize(&best_image, width, height, image::FilterType::Lanczos3);

                                let match_data = MatchData {
                                    x: x,
                                    y: y,
                                    tile: best_resize,
                                };
                                local_match_data.lock().unwrap().push(match_data);
                                tx.send(Some(1)).unwrap();
                            });
                        tx.send(None).unwrap();  
                    }));

                    let mut count = 0;
                    rx.attach(None, move |value| match value {
                        Some(_) => {
                            count += 1;
                            match_data_progress.set_text(Some(&(count.to_string() + " Tiles Placed")));
                            match_data_progress.set_fraction(count as f64 / total_tiles as f64);

                            glib::Continue(true)
                        }
                        None => {
                            let mut output = input.lock().unwrap().clone().take().unwrap();
                            for m in local_match_data.lock().unwrap().iter() {
                                replace(&mut output, &m.tile, m.x, m.y);
                            }
                            image::save_buffer(
                                &path,
                                &output,
                                output.width(),
                                output.height(),
                                image::ColorType::RGB(8),
                            )
                            .unwrap();

                            glib::Continue(false)
                        }
                    });
                }

                file_chooser.close();
            }));

            file_chooser.show_all();
        }));

        let container = gtk::Grid::new();
        container.attach(&pics_data_chooser_button, 0, 0, 1, 1);
        container.attach(&pics_data_progress, 1, 0, 1, 1);
        container.attach(&input_chooser_button, 0, 1, 1, 1);
        container.attach(&input_progress, 1, 1, 1, 1);
        container.attach(&output_chooser_button, 0, 2, 1, 1);
        container.attach(&match_data_progress, 1, 2, 1, 1);

        container.set_row_spacing(12);
        container.set_border_width(6);
        container.set_vexpand(true);
        container.set_hexpand(true);

        MainView {
            container,

            pics_data,
            pics_data_chooser_button,
            pics_data_progress,

            input,
            input_chooser_button,
            input_progress,

            output_chooser_button,
            match_data_progress,
        }
    }
}
