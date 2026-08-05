use eframe::egui::{self, FontId, TextWrapMode};
use eframe;
use std::thread;
use crate::{self as main, backend, save_cache, save_drives, save_settings};

#[derive(Debug, Default)]
struct Anything{
    items: (Vec<main::File>, Vec<main::Directory>),
    settings: main::Settings,
    drives: Vec<main::Drive>,
    searching_for: String,
    behaviour_window: bool,
    disk_window: bool,
    lsblk_window: bool,
    status: String,
    no_disk_popup: bool,
    edit_dir_popup: (bool,usize),
    temp: String,
    temp_drives: Vec<main::Drive>,
    indexed: bool,
    indexing_handle_thread: Option<std::thread::JoinHandle<(Vec<main::File>, Vec<main::Directory>, u32)>>,
    finished_indexing: bool,
    time_last_index: Option<std::time::Instant>,
    time_last_change: Option<std::time::Instant>,
    search_thread: Option<std::thread::JoinHandle<Vec<usize>>>,
    search_results: Vec<usize>,
    cancel_search: Option<std::sync::mpsc::Sender<u8>>,
    times_it_has_indexed: u32,
    time_to_index: f32,
    not_first_frame: bool,
    typing_name: String,
}

impl Anything{
    fn new(_cc: &eframe::CreationContext<'_>) -> Self{
        let mut app = Anything::default();
        app.settings = main::load_settings();
        app.drives = main::load_drives();
        if app.drives.len() == 0{
            app.no_disk_popup = true;
        }
        if app.settings.columns.len() == 0{
            app.settings.columns = vec![200, 950, 100, 150, 150]
        }
        app.items = main::load_cache();
        app.status = format!("{} indexed files!", app.items.0.len());
        app.temp = app.settings.index_every_minutes.to_string();
        app
    }
    fn sort_items(&mut self){
        match self.settings.sort_in_use{
            main::Sort::DateCreatedAscending => {
                self.items.0.sort_by(|a,b| a.create_timestamp.cmp(&b.create_timestamp));

            },
            main::Sort::DateCreatedDescending => {
                self.items.0.sort_by(|b,a| a.create_timestamp.cmp(&b.create_timestamp));

            },
            main::Sort::DateModifiedAscending => {
                self.items.0.sort_by(|a,b| a.last_modified_timestamp.cmp(&b.last_modified_timestamp));

            },
            main::Sort::DateModifiedDescending => {
                self.items.0.sort_by(|b,a| a.last_modified_timestamp.cmp(&b.last_modified_timestamp));

            },
            main::Sort::SizeAscending => {
                self.items.0.sort_by(|a,b| a.size.cmp(&b.size));

            },
            main::Sort::SizeDescending => {
                self.items.0.sort_by(|b,a| a.size.cmp(&b.size));

            },
            main::Sort::PathAscending => {
                self.items.0.sort_by(|a,b|
                    self.items.1[a.parent as usize].name.cmp(&self.items.1[b.parent as usize].name));


            },
            main::Sort::PathDescending => {
                self.items.0.sort_by(|b,a|
                    self.items.1[a.parent as usize].name.cmp(&self.items.1[b.parent as usize].name));

            },
            main::Sort::FileAscending => {
                self.items.0.sort_by(|a,b| a.name.cmp(&b.name));

            },
            main::Sort::FileDescending => {
                self.items.0.sort_by(|b,a| a.name.cmp(&b.name));

            },
        }
        // self.time_last_change = Some(std::time::Instant::now());
    }
    fn render_table(&mut self, ui: &mut egui::Ui) {
        const WH: &str = "                                                                      ";
        ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
        ui.style_mut().override_font_id = Some(FontId{size:16.0,family:egui::FontFamily::Proportional});
        let mut arrow = vec![String::new(); 5];
        match self.settings.sort_in_use{
            main::Sort::DateCreatedAscending => {arrow[2] = String::from("v")},
            main::Sort::DateCreatedDescending => {arrow[2] = String::from("^")},
            main::Sort::DateModifiedAscending => {arrow[3] = String::from("v")},
            main::Sort::DateModifiedDescending => {arrow[3] = String::from("^")},
            main::Sort::SizeAscending => {arrow[1] = String::from("v")},
            main::Sort::SizeDescending => {arrow[1] = String::from("^")},
            main::Sort::PathAscending => {arrow[4] = String::from("v")},
            main::Sort::PathDescending => {arrow[4] = String::from("^")},
            main::Sort::FileAscending => {arrow[0] = String::from("v")},
            main::Sort::FileDescending => {arrow[0] = String::from("^")},
        }
        use egui_extras::{TableBuilder, Column};
        TableBuilder::new(ui)
            .column(Column::initial(self.settings.columns[0] as f32).resizable(true))
            .column(Column::initial(self.settings.columns[2] as f32).resizable(true))
            .column(Column::initial(self.settings.columns[3] as f32).resizable(true))
            .column(Column::initial(self.settings.columns[4] as f32).resizable(true))
            .column(Column::remainder().resizable(true))
            .striped(true)
            .animate_scrolling(false)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
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
                            self.time_last_change = Some(std::time::Instant::now() - std::time::Duration::from_millis(300));
                        };
                    });
                });
                header.col(|ui| {
                    ui.horizontal(|ui|{
                        if ui.add_sized(ui.available_size(), egui::Button::new(format!("Size {}",&arrow[1]))).clicked(){
                            if self.settings.sort_in_use == main::Sort::SizeDescending{
                                self.settings.sort_in_use = main::Sort::SizeAscending;
                            }else{
                                self.settings.sort_in_use = main::Sort::SizeDescending;
                            }
                            self.sort_items();
                            self.time_last_change = Some(std::time::Instant::now() - std::time::Duration::from_millis(300));

                        };
                    });
                });
                header.col(|ui| {
                    ui.horizontal(|ui|{
                        if ui.add_sized(ui.available_size(), egui::Button::new(format!("Date Created {}",&arrow[2]))).clicked(){
                            if self.settings.sort_in_use == main::Sort::DateCreatedDescending{
                                self.settings.sort_in_use = main::Sort::DateCreatedAscending;
                            }else{
                                self.settings.sort_in_use = main::Sort::DateCreatedDescending;
                            }
                            self.sort_items();
                            self.time_last_change = Some(std::time::Instant::now() - std::time::Duration::from_millis(300));

                        };
                    });
                });
                header.col(|ui| {
                    ui.horizontal(|ui|{
                        if ui.add_sized(ui.available_size(), egui::Button::new(format!("Date Modified {}",&arrow[3]))).clicked(){
                            if self.settings.sort_in_use == main::Sort::DateModifiedDescending{
                                self.settings.sort_in_use = main::Sort::DateModifiedAscending;
                            }else{
                                self.settings.sort_in_use = main::Sort::DateModifiedDescending;
                            }
                            self.sort_items();
                            self.time_last_change = Some(std::time::Instant::now() - std::time::Duration::from_millis(300));

                        };
                    });
                });
                header.col(|ui| {
                    ui.horizontal(|ui|{
                        if ui.add_sized(ui.available_size(), egui::Button::new(
                            format!("Path {} {} {}{}{}{}{}",&arrow[4], WH, WH, WH, WH, WH, WH))).clicked(){
                            if self.settings.sort_in_use == main::Sort::PathDescending{
                                self.settings.sort_in_use = main::Sort::PathAscending;
                            }else{
                                self.settings.sort_in_use = main::Sort::PathDescending;
                            }
                            self.sort_items();
                            self.time_last_change = Some(std::time::Instant::now() - std::time::Duration::from_millis(300));

                        };
                    });
                });
            })
            .body(| body| {
                let n = if self.search_results.len() < 25{25} else{self.search_results.len() + 5};
                body.rows(24.0, n, |mut row| {
                    let row_index = row.index();
                    if row_index < self.search_results.len() && row_index < self.items.0.len(){
                        let i = self.search_results[row_index];
                        row.col(|ui| {
                            ui.label(&self.items.0[i].name);
                        });
                        row.col(|ui| {
                            ui.label(main::size_to_pretty_string(self.items.0[i].size));
                        });
                        row.col(|ui| {
                            ui.label(main::timestamp_to_string(self.items.0[i].create_timestamp));
                        });
                        row.col(|ui| {
                            ui.label(main::timestamp_to_string(self.items.0[i].last_modified_timestamp));
                        });
                        row.col(|ui| {
                            ui.label(self.items.1[self.items.0[i].parent as usize].name.clone()+&self.items.0[i].name);
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

impl eframe::App for Anything {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.not_first_frame{
            self.not_first_frame = true;
            if self.settings.light_mode {
                ctx.set_theme(egui::Theme::Light);
            } else {
                ctx.set_theme(egui::Theme::Dark);
            }
            if self.settings.pixels_per_point > 0.5 && self.settings.pixels_per_point < 100.0{
                ctx.set_pixels_per_point(self.settings.pixels_per_point);
            }
        }
        if self.time_last_change.is_some(){
            if self.cancel_search.is_some(){
                let _ = self.cancel_search.as_ref().unwrap().send(1);
            }
            if self.time_last_change.unwrap().elapsed() > std::time::Duration::from_millis(300) && self.time_last_index.unwrap().elapsed() > std::time::Duration::from_millis(3000){
                let (s, r) = std::sync::mpsc::channel::<u8>();
                self.cancel_search = Some(s);
                self.time_last_change = None;

                let ptr = self.items.0.as_ptr();
                let len = self.items.0.len();
                let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
                let ptr = self.items.1.as_ptr();
                let len = self.items.1.len();
                let slice_2 = unsafe { std::slice::from_raw_parts(ptr, len) };
                let settings_clone = self.settings.clone();
                let searching_for = self.searching_for.clone();
                let cancel_flag = r;
                self.search_thread = Some(thread::spawn(move || backend::search(slice.to_vec(),slice_2.to_vec(), settings_clone, searching_for, cancel_flag)));

                self.status = String::from("Searching...");
            }
        }

        if let Some(handle) = &self.search_thread{
            if handle.is_finished()  {
                if let Some(completed_handle) = self.search_thread.take() {
                    match completed_handle.join() {
                        Ok(res) => {
                            let mut size = 0;
                            for f in &res{
                                size += &self.items.0[*f].size;
                            }
                            self.status = format!("{} Files/Directories found. Size of all searched files: {}",res.len(), main::size_to_pretty_string(size));
                            self.search_results = res;
                            self.search_thread = None;
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
                self.indexing_handle_thread = Some(thread::spawn(|| backend::index_drives(d_clone)));
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
                                    self.items.0 = items.0;
                                    self.items.1 = items.1;
                                    self.sort_items();
                                    let errors = items.2;
                                    self.time_to_index = self.time_last_index.unwrap().elapsed().as_secs_f32();
                                    println!("Indexing took: {:.3?}", self.time_to_index);
                                    if errors == 0{
                                        self.status = format!("Indexing took: {:.3?}, Files found: {}"
                                            ,self.time_last_index.unwrap().elapsed(),self.items.0.len());
                                        self.time_last_change = Some(std::time::Instant::now());
                                    }else{
                                        let no_permission_or_doesnt_exist = errors % 100;
                                        let bad_magic = (errors - (errors % 100))/100;
                                        match (no_permission_or_doesnt_exist, bad_magic){
                                            (0,a) => {
                                                self.status = format!("Indexing took: {:.3?}, Files found: {}.\nWARNING: {} selected drives' magic header does not match selected filesystem type \nSearch something to continue"
                                                    ,self.time_last_index.unwrap().elapsed(),self.items.0.len(), a);
                                            },
                                            (a,0) => {
                                                self.status = format!("Indexing took: {:.3?}, Files found: {}.\nWARNING: {} drives do not exist or you do not have permission to open them\nSearch something to continue"
                                                    ,self.time_last_index.unwrap().elapsed(),self.items.0.len(), a);
                                            },
                                            (a,b) => {
                                                self.status = format!("Indexing took: {:.3?}, Files found: {}.\nWARNING: {} drives do not exist or you do not have permission to open them.\nWARNING: {} selected drives' magic header does not match selected filesystem type\nSearch something to continue "
                                                    ,self.time_last_index.unwrap().elapsed(),self.items.0.len(),a,b);
                                            },
                                        }
                                    }
                                    self.finished_indexing = true;
                                    self.times_it_has_indexed += 1;
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
            if self.settings.dynamic{
                let s = self.time_to_index as u64 * self.settings.dynamic_factor as u64;
                if s > 1{
                    if self.time_last_index.unwrap().elapsed() > std::time::Duration::from_secs(s) && self.finished_indexing{
                        self.indexed = false;
                    }
                }else{
                    if self.time_last_index.unwrap().elapsed() > std::time::Duration::from_secs(self.settings.dynamic_factor as u64) && self.finished_indexing{
                        self.indexed = false;
                    }
                }
            }else{
                let m = self.settings.index_every_minutes as u64;
                if std::time::Duration::from_secs(m * 60) >std::time::Duration::from_secs(1) &&
                self.time_last_index.unwrap().elapsed() > std::time::Duration::from_secs(m * 60){
                    self.indexed = false;
                }
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
        let mut open_info = self.edit_dir_popup;
        let mut s = String::new();
        if self.drives.len() != 0{
            s = format!("Editing ignored directories for drive {}", self.drives[open_info.1].drive);
        }
        egui::Window::new(s)
            .open(&mut open_info.0)
            .title_bar(true)
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.style_mut().override_font_id = Some(FontId{size:24.0,family:egui::FontFamily::Monospace});
                ui.vertical(|ui|{
                    let mut drives = self.drives.clone();
                    if drives[open_info.1].ignored_dirs.len() == 0{
                        ui.label("Start adding ignored directories!");
                    }
                    for (j, ignored) in self.drives[open_info.1].ignored_dirs.iter().enumerate(){
                        ui.horizontal(|ui|{
                            ui.label(ignored);
                            if ui.button("-" ).clicked(){
                                drives[open_info.1].ignored_dirs.remove(j);
                            }
                        });
                    }
                    if ui.text_edit_singleline(&mut self.typing_name).lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)){
                        drives[open_info.1].ignored_dirs.push(self.typing_name.clone());
                        self.typing_name = String::new()
                    }
                    self.drives = drives;
                    if ui.button("OK").clicked(){
                        self.edit_dir_popup = (false,0);
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
                    ui.checkbox(&mut self.settings.light_mode, "Light mode").changed().then(|| {
                        if self.settings.light_mode {
                            ctx.set_theme(egui::Theme::Light);
                        } else {
                            ctx.set_theme(egui::Theme::Dark);
                        }
                    });
                    ui.menu_button("Help", |ui|{
                        ui.style_mut().override_font_id = Some(FontId{size:20.0,family:egui::FontFamily::Monospace});
                        ui.label("Search Options:");
                        ui.label("Simple search:\n xyz zyx = contains \"xyz zyx\"");
                        ui.label("Complex search:\n \\ xyz = contains \"xyz\" (The space is necessary)");
                        ui.label("\\!xyz = doesn't contain \"xyz\"");
                        ui.label("\\*_xyz = ends with \"xyz\"");
                        ui.label("\\_*xyz = starts with \"xyz\"");
                        ui.label("\\!_*xyz = doesn't ends with \"xyz\"");
                        ui.label("\\!*_xyz = doesn't starts with \"xyz\"");
                        ui.label("\\ xyz\\ zyx =  Contains both \"xyz\" AND \"zyx\"");
                        ui.separator();
                        ui.label("For more information see:");
                        ui.hyperlink_to("Anything on github:\nhttps://github.com/davidevofficial/anything", "https://github.com/davidevofficial/anything")
                    });
                });
                if ui.button("🔄").clicked(){
                    self.indexed = true;
                    let d_clone = self.drives.clone();
                    self.indexing_handle_thread = Some(thread::spawn(|| backend::index_drives(d_clone)));
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
                           ui.checkbox(&mut new_settings.dynamic, "Dynamic Indexing");
                        });
                        if new_settings.dynamic{
                            ui.horizontal(|ui|{
                                let _slider = ui.add(egui::Slider::new(&mut new_settings.dynamic_factor, 2..=200).text("Dynamic Factor"));
                            });
                        }else{
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
                        }
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
                        let slider = ui.add(egui::Slider::new(&mut new_settings.pixels_per_point, 0.5..=2.0).text("UI Scale"));
                        // Only apply when the user releases the slider
                        if slider.drag_stopped() || slider.lost_focus() {
                            ctx.set_pixels_per_point(self.settings.pixels_per_point);
                        }

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
                            let mut removing = false;
                            ui.horizontal(|ui|{
                                    ui.label(drives[i].drive.clone()+"              ");
                                    ui.label(drives[i].mounted_at.clone()+"");
                                    if ui.button("-").clicked(){
                                        drives.remove(i);
                                        removing = true;
                                    }
                                    if ui.button("\u{270F}").clicked(){
                                        self.edit_dir_popup = (true,i);
                                    }
                            });
                            if removing{
                                break;
                            }
                        }
                        ui.horizontal(|ui|{
                            if ui.add_sized(ui.available_size(), egui::Button::new("+")).clicked(){
                                self.temp_drives = backend::get_devices();
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
        if self.times_it_has_indexed > 0 && self.items.0.len() > 0 &&  self.items.1.len() > 0{
            save_cache(self.items.0.clone(),self.items.1.clone());
        }
        println!("Bye Bye");
    }
}
pub fn start_frontend() -> Result<(), eframe::Error>{
    let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1600.0, 700.0])
                .with_title("Anything - v2.2")
                .with_icon(
                    // NOTE: Adding an icon is optional
                    eframe::icon_data::from_png_bytes(&include_bytes!("../icon.png")[..])
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