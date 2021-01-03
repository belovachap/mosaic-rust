use gio::prelude::*;
use glib::clone;
use gtk::prelude::*;

use std::cell::RefCell;
use std::env::args;
use std::rc::Rc;
use std::thread;

use image::imageops::resize;

use std::fs;
use std::path::{Path, PathBuf};

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
    pub view_stack: gtk::Stack,
    pub main_view: MainView,
    pub complete_view: CompleteView,
}

impl Widgets {
    pub fn new(application: &gtk::Application) -> Self {
        let complete_view = CompleteView::new();
        let main_view = MainView::new();

        let view_stack = gtk::Stack::new();
        view_stack.set_border_width(6);
        view_stack.set_vexpand(true);
        view_stack.set_hexpand(true);
        view_stack.add(&main_view.container);
        view_stack.add(&complete_view.container);

        let header = Header::new();

        let window = gtk::ApplicationWindow::new(application);
        window.set_icon_name(Some("package-x-generic"));
        window.set_property_window_position(gtk::WindowPosition::Center);
        window.set_titlebar(Some(&header.container));
        window.add(&view_stack);
        window.show_all();
        window.set_default_size(500, 250);
        window.connect_delete_event(move |window, _| {
            window.close();
            Inhibit(false)
        });

        Widgets {
            window,
            header,
            view_stack,
            main_view,
            complete_view,
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

pub struct CompleteView {
    pub container: gtk::Grid,
}

impl CompleteView {
    pub fn new() -> Self {
        let label = gtk::Label::new(None);
        label.set_markup("Task complete");
        label.set_halign(gtk::Align::Center);
        label.set_valign(gtk::Align::Center);
        label.set_vexpand(true);
        label.set_hexpand(true);

        let container = gtk::Grid::new();
        container.set_vexpand(true);
        container.set_hexpand(true);
        container.add(&label);

        CompleteView { container }
    }
}

pub struct MainView {
    pub container: gtk::Grid,
    pub progress: gtk::ProgressBar,
    pub button: gtk::Button,
    pub folder_chooser_button: gtk::FileChooserButton,
    pub file_chooser_button: gtk::FileChooserButton,
    pub pics_data: Rc<RefCell<Vec<PicData>>>,
}

pub fn get_pic_data(path: PathBuf) -> PicData {
    let img = image::open(&path).unwrap().to_rgb();
    let aspect = img.width() as f64 / img.height() as f64;
    let thumbnail = resize(&img, 128, 128, image::FilterType::Lanczos3);
    
    PicData{path: path, aspect: aspect, thumbnail: thumbnail}
}

pub fn get_pics_data(pics_dir: &Path, tx: glib::Sender<Option<PicData>>) {
    let paths = fs::read_dir(pics_dir).unwrap().map(|x| x.unwrap().path());
    paths.par_bridge().for_each(|path| { tx.send(Some(get_pic_data(path))).unwrap(); });
}


impl MainView {
    pub fn new() -> Self {
        let progress = gtk::ProgressBar::new();
        progress.set_text(Some("0 Pictures Loaded"));
        progress.set_show_text(true);
        progress.set_hexpand(true);

        let button = gtk::Button::new();
        button.set_label("start");
        button.set_halign(gtk::Align::Center);

	let pics_data = Rc::new(RefCell::new(Vec::new()));
        let folder_chooser_button = gtk::FileChooserButton::new("Select Pictures", gtk::FileChooserAction::SelectFolder);
        folder_chooser_button.connect_file_set(clone!(@weak pics_data, @weak progress => move |button| {
            let path = button.get_filename().unwrap();
            println!("You selected: {:?}", path);

	    let total_files = fs::read_dir(&path).unwrap().count();
            println!("Total files: {:?}", total_files);
    
            let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
            
            thread::spawn(move || { get_pics_data(&path, tx); });

            let mut count = 0;
            rx.attach(None, move |value| match value {
                Some(pic_data) => {
                    count += 1;
                    progress.set_text(Some(&(count.to_string() + " Pictures Loaded")));
                    progress.set_fraction(count as f64 / total_files as f64);
                    pics_data.borrow_mut().push(pic_data);
                    
                    glib::Continue(true)
                }
                None => {

                    glib::Continue(false)
                }
            });
        }));

        let file_chooser_button = gtk::FileChooserButton::new("Select Picture", gtk::FileChooserAction::Open);
        file_chooser_button.connect_file_set(clone!(@weak pics_data => move |button| {
            let path = button.get_filename().unwrap();
            println!("You selected: {:?}", path);
            println!("pics_data has {} pics", pics_data.borrow().len());
        }));

        let container = gtk::Grid::new();
        container.attach(&folder_chooser_button, 0, 0, 1, 1);        
        container.attach(&progress, 1, 0, 1, 1);
        container.attach(&file_chooser_button, 0, 1, 1, 1);
        
        container.set_row_spacing(12);
        container.set_border_width(6);
        container.set_vexpand(true);
        container.set_hexpand(true);

        MainView {
            container,
            progress,
            button,
            folder_chooser_button,
            pics_data,
            file_chooser_button,
        }
    }
}
