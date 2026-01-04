use eframe::egui::{self, FontId, TextWrapMode};
use eframe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::sync::Arc;
use crate::{self as main, SupportedFilesystems, save_cache, save_drives, save_settings};

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
    time_last_change: Option<std::time::Instant>,
    search_thread1: Option<std::thread::JoinHandle<Vec<main::File>>>,
    search_thread2: Option<std::thread::JoinHandle<Vec<main::File>>>,
    search_results: Vec<main::File>,
    cancel_search: Arc<AtomicBool>
}

impl Anything{
    fn new(_cc: &eframe::CreationContext<'_>) -> Self{
        let mut app = Anything::default();
        app.settings = main::load_settings();
        app.drives = main::load_drives();
        if app.drives.len() == 0{
            app.no_disk_popup = true;
        }
        app.items = main::load_cache();
        app.temp = app.settings.index_every_minutes.to_string();
        app
    }
    fn sort_items(&mut self){
        match self.settings.sort_in_use{
            main::Sort::DateCreatedAscending => {
                self.items.sort_by(|a,b| a.create_timestamp.cmp(&b.create_timestamp));
                self.search_results.sort_by(|a,b| a.create_timestamp.cmp(&b.create_timestamp));

            },
            main::Sort::DateCreatedDescending => {
                self.items.sort_by(|b,a| a.create_timestamp.cmp(&b.create_timestamp));
                self.search_results.sort_by(|b,a| a.create_timestamp.cmp(&b.create_timestamp));
            },
            main::Sort::DateModifiedAscending => {
                self.items.sort_by(|a,b| a.last_modified_timestamp.cmp(&b.last_modified_timestamp));
                self.search_results.sort_by(|a,b| a.last_modified_timestamp.cmp(&b.last_modified_timestamp));
            },
            main::Sort::DateModifiedDescending => {
                self.items.sort_by(|b,a| a.last_modified_timestamp.cmp(&b.last_modified_timestamp));
                self.search_results.sort_by(|b,a| a.last_modified_timestamp.cmp(&b.last_modified_timestamp));
            },
            main::Sort::SizeAscending => {
                self.items.sort_by(|a,b| a.size.cmp(&b.size));
                self.search_results.sort_by(|a,b| a.size.cmp(&b.size));
            },
            main::Sort::SizeDescending => {
                self.items.sort_by(|b,a| a.size.cmp(&b.size));
                self.search_results.sort_by(|b,a| a.size.cmp(&b.size));
            },
            main::Sort::PathAscending => {
                self.items.sort_by(|a,b| a.full_name.cmp(&b.full_name));
                self.search_results.sort_by(|a,b| a.full_name.cmp(&b.full_name));
            },
            main::Sort::PathDescending => {
                self.items.sort_by(|b,a| a.full_name.cmp(&b.full_name));
                self.search_results.sort_by(|b,a| a.full_name.cmp(&b.full_name));
            },
            main::Sort::FileAscending => {
                self.items.sort_by(|a,b| a.name.cmp(&b.name));
                self.search_results.sort_by(|a,b| a.name.cmp(&b.name));
            },
            main::Sort::FileDescending => {
                self.items.sort_by(|b,a| a.name.cmp(&b.name));
                self.search_results.sort_by(|b,a| a.name.cmp(&b.name));
            },
        }
    }
    fn render_table(&mut self, ui: &mut egui::Ui) {

        ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
        ui.style_mut().override_font_id = Some(FontId{size:16.0,family:egui::FontFamily::Proportional});
        let mut arrow = vec![String::new(); 5];
        match self.settings.sort_in_use{
            main::Sort::DateCreatedAscending => {arrow[3] = String::from("v")},
            main::Sort::DateCreatedDescending => {arrow[3] = String::from("^")},
            main::Sort::DateModifiedAscending => {arrow[4] = String::from("v")},
            main::Sort::DateModifiedDescending => {arrow[4] = String::from("^")},
            main::Sort::SizeAscending => {arrow[2] = String::from("v")},
            main::Sort::SizeDescending => {arrow[2] = String::from("^")},
            main::Sort::PathAscending => {arrow[1] = String::from("v")},
            main::Sort::PathDescending => {arrow[1] = String::from("^")},
            main::Sort::FileAscending => {arrow[0] = String::from("v")},
            main::Sort::FileDescending => {arrow[0] = String::from("^")},
        }
        use egui_extras::{TableBuilder, Column};
        TableBuilder::new(ui)
            .column(Column::initial(self.settings.columns[0] as f32).resizable(true))
            .column(Column::initial(self.settings.columns[1] as f32).resizable(true))
            .column(Column::initial(self.settings.columns[2] as f32).resizable(true))
            .column(Column::initial(self.settings.columns[3] as f32).resizable(true))
            .column(Column::initial(self.settings.columns[4] as f32).resizable(true))
            .striped(true)
            .drag_to_scroll(true)
            .header(24.0, |mut header| {
                header.col(|ui| {
                    ui.horizontal(|ui|{
                        if ui.add_sized(ui.available_size(), egui::Button::new(format!("Name {}",&arrow[0]))).clicked(){
                            if self.settings.sort_in_use == main::Sort::FileDescending{
                                self.settings.sort_in_use = main::Sort::FileAscending;
                            }else{
                                self.settings.sort_in_use = main::Sort::FileDescending;
                            }
                            self.sort_items();
                        };
                    });
                });
                header.col(|ui| {
                    ui.horizontal(|ui|{
                        if ui.add_sized(ui.available_size(), egui::Button::new(format!("Path {}",&arrow[1]))).clicked(){
                            if self.settings.sort_in_use == main::Sort::PathDescending{
                                self.settings.sort_in_use = main::Sort::PathAscending;
                            }else{
                                self.settings.sort_in_use = main::Sort::PathDescending;
                            }
                            self.sort_items();

                        };
                    });
                });
                header.col(|ui| {
                    ui.horizontal(|ui|{
                        if ui.add_sized(ui.available_size(), egui::Button::new(format!("Size {}",&arrow[2]))).clicked(){
                            if self.settings.sort_in_use == main::Sort::SizeDescending{
                                self.settings.sort_in_use = main::Sort::SizeAscending;
                            }else{
                                self.settings.sort_in_use = main::Sort::SizeDescending;
                            }
                            self.sort_items();

                        };
                    });
                });
                header.col(|ui| {
                    ui.horizontal(|ui|{
                        if ui.add_sized(ui.available_size(), egui::Button::new(format!("Date Created {}",&arrow[3]))).clicked(){
                            if self.settings.sort_in_use == main::Sort::DateCreatedDescending{
                                self.settings.sort_in_use = main::Sort::DateCreatedAscending;
                            }else{
                                self.settings.sort_in_use = main::Sort::DateCreatedDescending;
                            }
                            self.sort_items();

                        };
                    });
                });
                header.col(|ui| {
                    ui.horizontal(|ui|{
                        if ui.add_sized(ui.available_size(), egui::Button::new(format!("Date Modified {}",&arrow[4]))).clicked(){
                            if self.settings.sort_in_use == main::Sort::DateModifiedDescending{
                                self.settings.sort_in_use = main::Sort::DateModifiedAscending;
                            }else{
                                self.settings.sort_in_use = main::Sort::DateModifiedDescending;
                            }
                            self.sort_items();

                        };
                    });
                });
            })
            .body(|mut body| {
                body.rows(24.0, self.search_results.len()+5, |mut row| {
                    let row_index = row.index();
                    if row_index < self.search_results.len(){
                        row.col(|ui| {
                            ui.label(format!("{}",self.search_results[row_index].name.clone()));
                        });
                        row.col(|ui| {
                            ui.label(format!("{}",self.search_results[row_index].full_name.clone()));
                        });
                        row.col(|ui| {
                            ui.label(main::size_to_pretty_string(self.search_results[row_index].size));
                        });
                        row.col(|ui| {
                            ui.label(main::timestamp_to_string(self.search_results[row_index].create_timestamp));
                        });
                        row.col(|ui| {
                            ui.label(main::timestamp_to_string(self.search_results[row_index].last_modified_timestamp));
                        });
                    }else{
                        row.col(|_ui|{});
                        row.col(|_ui|{});
                        row.col(|_ui|{});
                        row.col(|_ui|{});
                        row.col(|_ui|{});
                    }
                });
            });
    }


}
fn convert_string_to_predicates(searching_for: String)->Vec<String>{
    vec![searching_for]
}
fn search_30(items: Vec<main::File>, settings: main::Settings, searching_for: String)->Vec<main::File>{
    let mut output: Vec<main::File> = Vec::new();
    let pred = convert_string_to_predicates(searching_for.clone());
    for p in pred{
        if output.len() == 30{
            break;
        }
        if output.len() == 0{
            for item in 0..items.len(){
                let f: main::File = items[item].clone();
                let mut n = String::new();
                let mut m = p.clone();
                if settings.search_full_path {
                    n = f.full_name.clone();
                }else{
                    n = f.name.clone();
                }
                if settings.ignore_case{
                    n = n.to_lowercase();
                    m = m.to_lowercase();
                }
                if n == m{
                    output.push(f);
                }
            }
        }
        else{
            for f in 0..output.len(){
                let f: main::File = items[f].clone();
                let mut n = String::new();
                let mut m = p.clone();
                if settings.search_full_path {
                    n = f.full_name.clone();
                }
                if settings.ignore_case{
                    n = n.to_lowercase();
                    m = m.to_lowercase();
                }
                if n.contains(&m){
                    output.push(f);
                }
            }
        }

    }
    dbg!(searching_for);
    output
}
fn search(items: Vec<main::File>, settings: main::Settings, searching_for: String,cancel_flag: &Arc<AtomicBool>)->Vec<main::File>{
    let mut output: Vec<main::File> = Vec::new();
    let pred = convert_string_to_predicates(searching_for.clone());
    for p in pred{
        if cancel_flag.load(Ordering::Relaxed){
            println!("Interrupted");
            return Vec::new();
        }
        if output.len() == 0{
            for item in 0..items.len(){
                let f: main::File = items[item].clone();
                let mut n = String::new();
                let mut m = searching_for.clone();
                if settings.search_full_path {
                    n = f.full_name.clone();
                }else{
                    n = f.name.clone();
                }
                if settings.ignore_case{
                    n = n.to_lowercase();
                    m = m.to_lowercase();
                }
                if n.contains(&m){
                    output.push(f);
                }
            }
        }
        else{
            for f in 0..output.len(){
                let f: main::File = items[f].clone();
                let mut n = String::new();
                let mut m = searching_for.clone();
                if settings.search_full_path {
                    n = f.full_name.clone();
                }
                if settings.ignore_case{
                    n = n.to_lowercase();
                    m = m.to_lowercase();
                }
                if n == m{
                    output.push(f);
                }
            }
        }
    }
    println!("Fully Searched");
    output
}
fn index_drives(drives: Vec<main::Drive>)->Vec<main::File>{
    let mut items = Vec::new();
    for d in drives.clone(){
        match d.fs{
            SupportedFilesystems::Exfat => {
                items.append(&mut main::exfat::index(d.drive, d.mounted_at, d.ignored_dirs));
            }
        }
    }
    items
}
impl eframe::App for Anything {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.time_last_change.is_some(){
            if self.time_last_change.unwrap().elapsed() > std::time::Duration::from_millis(300){
                self.time_last_change = None;
                self.cancel_search.store(true, Ordering::Relaxed);
                // Spawn thread 1
                let ptr = self.items.as_ptr();
                let len = self.items.len();
                let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
                let settings_clone = self.settings.clone();
                let searching_for = self.searching_for.clone();
                self.search_thread1 = Some(thread::spawn(move ||search_30(slice.to_vec(), settings_clone, searching_for)));

                // Spawn thread 2
                let settings_clone = self.settings.clone();
                let searching_for = self.searching_for.clone();
                self.cancel_search.store(false, Ordering::Relaxed);
                let cancel_flag = self.cancel_search.clone();
                self.cancel_search = cancel_flag.clone();
                self.search_thread2 = Some(thread::spawn(move ||search(slice.to_vec(), settings_clone, searching_for, &cancel_flag)));

                self.status = String::from("Searching...");
            }
        }
        if let Some(handle) = &self.search_thread1 {
            if handle.is_finished() {
                if let Some(completed_handle) = self.search_thread1.take() {
                    match completed_handle.join() {
                        Ok(res) => {self.search_results = res; self.search_thread1 = None}
                        Err(_) => {self.status = String::from("Searching Failed...")}
                    }
                }
            }
        }
        if let Some(handle) = &self.search_thread2 {
            if handle.is_finished()  {
                if let Some(completed_handle) = self.search_thread2.take() {
                    match completed_handle.join() {
                        Ok(res) => {
                            self.status = format!("{} Files/Directories found",res.len());
                            self.search_results = res;
                            self.search_thread2 = None;
                        }
                        Err(_) => {self.status = String::from("Searching Interrupted or Failed...")}
                    }
                }
            }
        }

        if !self.settings.index_on_startup && self.time_last_index.is_none(){
            self.indexed = true
        }

        if !self.indexed{
            if self.status != String::from("Searching..."){
                self.indexed = true;
                let d_clone = self.drives.clone();
                self.indexing_handle_thread = Some(thread::spawn(||index_drives(d_clone)));
                self.finished_indexing = false;
                self.time_last_index = Some(std::time::Instant::now());
            }
        }

        if let Some(handle) = &self.indexing_handle_thread {
                    self.status = String::from("Indexing...");
                    if handle.is_finished() && !self.finished_indexing {
                        if let Some(completed_handle) = self.indexing_handle_thread.take() {
                            match completed_handle.join() {
                                Ok(items) => {
                                    self.items = items;
                                    self.sort_items();
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
                    self.time_last_change = Some(std::time::Instant::now());
                }
                if ui.add(egui::TextEdit::singleline(&mut self.searching_for)
                    .desired_width(ui.available_width() * 1.0)).changed(){
                        if self.settings.instant_search{
                            self.time_last_change = Some(std::time::Instant::now());
                        }
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
                            ui.checkbox(&mut new_settings.search_full_path, "Search Full Path");
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

                                // let before = drives[i].fs;
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
            ui.style_mut().override_font_id = Some(FontId{size:20.0,family:egui::FontFamily::Proportional});
            ui.label(self.status.clone());
        });

        ctx.request_repaint_after_secs(0.1);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        save_settings(self.settings.clone());
        save_drives(self.drives.clone());
        save_cache(self.items.clone()); //todo!()
        println!("Bye Bye");
    }
}
pub fn start_frontend() -> Result<(), eframe::Error>{
    let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1600.0, 700.0])
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