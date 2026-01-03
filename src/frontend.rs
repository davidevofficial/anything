use eframe::egui::{self, FontId, InnerResponse, Response, TextWrapMode, UiBuilder};
use eframe;
use egui_extras::{Column, TableBuilder};
use std::path::PathBuf;
use std::thread;
use crate::{self as main, SupportedFilesystems, save_cache, save_drives, save_settings};
// ↓ ↑
// todo!() add combobox filesystem type
#[derive(Debug, Default)]
struct Anything{
    items: Vec<main::File>,
    settings: main::Settings,
    drives: Vec<main::Drive>,
    searching_for: String,
    behaviour_window: bool,
    disk_window: bool,
    lsblk_window: bool,
    status: String,
    no_disk_popup: bool,
    info_popup: bool,
    temp: String,
    temp_drives: Vec<main::Drive>,
    indexed: bool,
    indexing_handle_thread: Option<std::thread::JoinHandle<Vec<main::File>>>,
    finished_indexing: bool,
    time_last_index: Option<std::time::Instant>,
}

impl Anything{
    fn new(_cc: &eframe::CreationContext<'_>) -> Self{
        let mut app = Anything::default();
        app.settings = main::load_settings();
        app.drives = main::load_drives();
        if app.drives.len() == 0{
            app.no_disk_popup = true;
        }
        app.temp = app.settings.index_every_minutes.to_string();
        app
    }
    fn render_table(&mut self, ui: &mut egui::Ui) {

        ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);

        use egui_extras::{TableBuilder, Column};
        TableBuilder::new(ui)
            .column(Column::auto().resizable(true))
            .column(Column::remainder())
            .striped(true)
            .drag_to_scroll(true)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.heading("First column");
                });
                header.col(|ui| {
                    ui.heading("Second column");
                });
            })
            .body(|mut body| {
                body.row(30.0, |mut row| {
                    row.col(|ui| {
                        ui.label("Hello");
                    });
                    row.col(|ui| {
                        ui.button("world!");
                    });
                });
            });
    }

}
fn index_drives(drives: Vec<main::Drive>)->Vec<main::File>{
    let mut items = Vec::new();
    for d in drives.clone(){
        match d.fs{
            SupportedFilesystems::Exfat => {
                dbg!(d.clone());
                items.append(&mut main::exfat::index(d.drive, d.mounted_at, d.ignored_dirs));
            }
        }
    }
    items
}
impl eframe::App for Anything {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {

        if !self.settings.index_on_startup && self.time_last_index.is_none(){
            self.indexed = true
        }

        if self.settings.index_on_startup && !self.indexed{
            self.indexed = true;
            let d_clone = self.drives.clone();
            self.indexing_handle_thread = Some(thread::spawn(||index_drives(d_clone)));
            self.finished_indexing = false;
            self.time_last_index = Some(std::time::Instant::now());
        }

        if let Some(handle) = &self.indexing_handle_thread {
                    self.status = String::from("Indexing...");
                    if handle.is_finished() && !self.finished_indexing {
                        if let Some(completed_handle) = self.indexing_handle_thread.take() {
                            match completed_handle.join() {
                                Ok(items) => {
                                    self.items = items;
                                    self.status = format!("Indexing took: {:.3?}, Files found: {}"
                                        ,self.time_last_index.unwrap().elapsed(),self.items.len());
                                    self.finished_indexing = true;
                                }
                                Err(_) => {
                                    self.status = String::from("Indexing failed: probably because of lacking permission or a drive didn't exist");
                                    self.finished_indexing = true;
                                }
                            }
                        }
                    }
                }
        if self.time_last_index.is_none(){
            self.time_last_index = Some(std::time::Instant::now());
        } else {
            let m = self.settings.index_every_minutes as u64;
            if std::time::Duration::from_secs(m * 60) >std::time::Duration::from_secs(1) &&
            self.time_last_index.unwrap().elapsed() > std::time::Duration::from_secs(m * 60){
                self.indexed = false;
            }
        }
        // No disk warning
        let mut open_warning = self.no_disk_popup;
        egui::Window::new("Warning!")
            .open(&mut open_warning)
            .title_bar(true)
            .resizable(false)
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.style_mut().override_font_id = Some(FontId{size:24.0,family:egui::FontFamily::Monospace});
                    ui.vertical(|ui|{
                        ui.label("Warning No disk selected!");
                        if ui.button("OK").clicked(){
                            self.no_disk_popup = false;
                        }
                    });
        });
        // Edit popup explanation
        let mut open_info = self.info_popup;
        egui::Window::new("Information!")
            .open(&mut open_info)
            .title_bar(true)
            .resizable(false)
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.style_mut().override_font_id = Some(FontId{size:24.0,family:egui::FontFamily::Monospace});
                    ui.vertical(|ui|{
                        ui.label("Information: Check the readme on github on how to edit ignored directories!");
                        if ui.button("OK").clicked(){
                            self.info_popup = false;
                        }
                    });
        });



        // Top search bar
        egui::TopBottomPanel::top("search_bar").show(ctx, |ui| {
            ui.style_mut().override_font_id = Some(FontId{size:27.0,family:egui::FontFamily::Monospace});
            ui.horizontal(|ui| {
                ui.menu_button("\u{2699}", |ui| {
                    ui.style_mut().override_font_id = Some(FontId{size:27.0,family:egui::FontFamily::Monospace});

                    if ui.button("Behaviour").clicked(){
                        self.behaviour_window = true;
                    };

                    ui.style_mut().override_font_id = Some(FontId{size:27.0,family:egui::FontFamily::Monospace});
                    if ui.button("Disks").clicked() {
                        self.disk_window = true;
                    }
                });
                if ui.button("🔄").clicked(){
                    self.indexed = true;
                    let d_clone = self.drives.clone();
                    self.indexing_handle_thread = Some(thread::spawn(||index_drives(d_clone)));
                    self.finished_indexing = false;
                    self.time_last_index = Some(std::time::Instant::now());
                }
                if ui.small_button("🔎").clicked(){
                    println!("aura")
                }
                if ui.add(egui::TextEdit::singleline(&mut self.searching_for)
                    .desired_width(ui.available_width() * 1.0)).changed(){
                        println!("Bro is typing");
                };
            });
        });
        let mut open = self.behaviour_window;
        let mut new_settings = self.settings.clone();
        let mut temp = self.temp.clone();
        egui::Window::new("Behaviour Settings")
                    .open(&mut open)
                    .title_bar(true)
                    .resizable(false)
                    .default_width(450.0)
                    .show(ctx, |ui| {
                        ui.style_mut().override_font_id = Some(FontId{size:24.0,family:egui::FontFamily::Monospace});
                        // create buttons and change new_settings
                        ui.horizontal(|ui|{
                            ui.checkbox(&mut new_settings.index_on_startup, "Index on startup");
                        });
                        ui.horizontal(|ui|{
                            ui.label("Index Once every");
                            if ui.text_edit_singleline(&mut temp).changed(){
                                self.temp = temp.clone();
                                let try_convert = temp.parse::<u32>();
                                if try_convert.is_ok(){
                                    new_settings.index_every_minutes = try_convert.unwrap();

                                }
                            }
                            ui.label("Minutes");
                        });
                        ui.horizontal(|ui|{
                            ui.checkbox(&mut new_settings.instant_search, "Istant Search");
                        });
                        ui.horizontal(|ui|{
                            ui.checkbox(&mut new_settings.journal, "Journal");
                        });
                        ui.horizontal(|ui|{
                            ui.checkbox(&mut new_settings.ignore_case, "Ignore Case");
                        });

                        ui.horizontal(|ui|{
                            if ui.add_sized(ui.available_size(), egui::Button::new("Ok")).clicked(){
                                self.behaviour_window = false;
                            };
                        });
                    });
        self.settings = new_settings;

        let mut open_disk_window = self.disk_window;
        let mut drives = self.drives.clone();
        egui::Window::new("Drive Settings")
                    .open(&mut open_disk_window)
                    .title_bar(true)
                    .resizable(false)
                    .default_width(600.0)
                    .show(ctx, |ui| {
                        ui.style_mut().override_font_id = Some(FontId{size:24.0,family:egui::FontFamily::Monospace});
                        // create buttons and change new_settings
                        for i in 0..drives.len(){
                            ui.horizontal(|ui|{
                                ui.label(drives[i].drive.clone()+"    ");
                                ui.label(drives[i].mounted_at.clone()+"    ");

                                let before = drives[i].fs;
                                egui::ComboBox::new(drives[i].drive.clone(),"")
                                    .selected_text(format!("{:?}", drives[i].fs))
                                    .show_ui(ui, |ui| {
                                        ui.style_mut().override_font_id = Some(FontId{size:24.0,family:egui::FontFamily::Monospace});
                                        ui.selectable_value(&mut drives[i].fs, SupportedFilesystems::Exfat, "Exfat");

                                    }
                                );
                                // if drives[i].fs != before {}

                                if ui.button("-").clicked(){
                                    drives.remove(i);
                                }
                                if ui.button("\u{270F}").clicked(){
                                    self.info_popup = true;
                                }
                            });
                        }
                        ui.horizontal(|ui|{
                            if ui.add_sized(ui.available_size(), egui::Button::new("+")).clicked(){
                                self.temp_drives = main::get_devices();
                                self.lsblk_window = true;
                            };
                        });
                        ui.horizontal(|ui|{
                            if ui.add_sized(ui.available_size(), egui::Button::new("Ok")).clicked(){
                                self.disk_window = false;
                            };
                        });

                    });
        self.drives = drives;

        let mut open_lsblk_window = self.lsblk_window;
        egui::Window::new("lsblk output")
                    .open(&mut open_lsblk_window)
                    .title_bar(true)
                    .resizable(false)
                    .default_width(500.0)
                    .show(ctx, |ui| {
                        ui.style_mut().override_font_id = Some(FontId{size:24.0,family:egui::FontFamily::Monospace});
                        // create buttons and change new_settings
                        for i in 0..self.temp_drives.len(){
                            let mut unique = true;
                            for j in 0..self.drives.len(){
                                if self.drives[j].mounted_at == self.temp_drives[i].mounted_at{
                                    unique = false;
                                }
                            }
                            if unique{
                                ui.horizontal(|ui|{
                                    if ui.button(self.temp_drives[i].drive.clone()+"    ").clicked(){
                                        self.drives.push(self.temp_drives[i].clone());
                                    };
                                    ui.label(self.temp_drives[i].mounted_at.clone()+"    ");
                                });
                            }
                        }

                        ui.horizontal(|ui|{
                            if ui.add_sized(ui.available_size(), egui::Button::new("Ok")).clicked(){
                                save_drives(self.drives.clone());
                                self.lsblk_window = false;
                            };
                        });

                    });

        // Main table
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_table(ui);
        });

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.style_mut().override_font_id = Some(FontId{size:24.0,family:egui::FontFamily::Monospace});
            ui.label(self.status.clone());
        });

        ctx.request_repaint_after_secs(0.1);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        save_settings(self.settings.clone());
        save_drives(self.drives.clone());
        // save_cache(self.items); //todo!()
        println!("Bye Bye");
    }
}
pub fn start_frontend() -> Result<(), eframe::Error>{
    let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1200.0, 800.0])
                .with_title("Anything")
                .with_icon(
                    // NOTE: Adding an icon is optional
                    eframe::icon_data::from_png_bytes(&include_bytes!("../settings/icon.png")[..])
                        .expect("Failed to load icon"),
                ),
            ..Default::default()
        };

        eframe::run_native(
            "Anything",
            options,
            Box::new(|cc| Ok(Box::new(Anything::new(cc)))),
        )
}