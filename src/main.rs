#![warn(clippy::pedantic)]
#![warn(clippy::implicit_return)]
#![allow(clippy::needless_return)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dirs::home_dir;
use eframe::{egui, icon_data, NativeOptions};
use eframe::egui::{Context, vec2, ViewportBuilder};
use sysinfo::System;

use crate::downloader::fetch_latest_weave_url;

mod downloader;
mod launcher;

#[derive(Clone)]
struct LunarProcess {
    pid: u32,
    exe: String,
    launch_cmd_modified: Vec<String>,
    weave_installed: bool,
    flatpak: bool,
    home_path: PathBuf,
}

#[derive(Clone)]
struct App {
    lunar_client: Option<LunarProcess>,
    weaver_path: String,
    downloading: bool,
    lunar_weave_ready: (bool, bool), // Tuple indicating if the game is ready, the first one is for Lunar and the second one is for Weave
    log_messages: Arc<Mutex<Vec<String>>>, // Shared log messages
}

impl App {
    fn new() -> Self {
        let weaver = get_weave_loader();
        let weaver_path = weaver.1.to_str().unwrap();
        let lunar_client = fetch_lunar_client(weaver_path);

        return App {
            lunar_client,
            weaver_path: weaver_path.to_string(),
            downloading: false,
            lunar_weave_ready: (false, false), // Initially not ready
            log_messages: Arc::new(Mutex::new(vec![])), // Initialize log messages
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.lunar_client = fetch_lunar_client(&self.weaver_path);

        egui::CentralPanel::default().show(ctx, |ui| {
            // Main layout
            ui.horizontal(|ui| {
                // Left panel for status information
                ui.vertical(|ui| {
                    ui.heading("Status");
                    ui.add_space(5.0);

                    // Lunar Client status
                    if let Some(lunar_client) = &self.lunar_client {
                        ui.colored_label(egui::Color32::GREEN, "✓ Lunar Client found");
                        ui.label(format!("PID: {}", lunar_client.pid));
                        ui.label(format!("Home: {:?}", lunar_client.home_path));

                        if lunar_client.flatpak {
                            ui.colored_label(egui::Color32::from_rgb(253, 218, 13), "⚠ Flatpak detected");
                            ui.label("Ensure ~/.weave is exposed to the sandbox.");
                        }

                        self.lunar_weave_ready.0 = true;
                    } else {
                        ui.colored_label(egui::Color32::RED, "✗ Minecraft not found");
                        ui.label("Launch the game from Lunar Client.");
                        self.lunar_weave_ready.0 = false;
                    }

                    ui.add_space(10.0);

                    // Weave Loader status
                    let (weave_exists, weave_loader) = get_weave_loader();
                    if weave_exists {
                        ui.colored_label(egui::Color32::GREEN, "✓ Weave Loader found");
                        ui.label(format!("Path: {weave_loader:?}"));
                        self.downloading = false;
                        self.lunar_weave_ready.1 = true;
                    } else {
                        ui.colored_label(egui::Color32::RED, "✗ Weave Loader not found");
                        ui.label(format!("Expected at: {weave_loader:?}"));

                        // Download button
                        if !self.downloading && ui.button("Download Weave Loader").clicked() {
                            self.downloading = true;
                            let log_messages = self.log_messages.clone();
                            tokio::spawn(async move {
                                let url = fetch_latest_weave_url("Weave-MC", "Weave-Loader").await.unwrap().unwrap();
                                downloader::download_jar(&url, &weave_loader).await.unwrap();
                                log_messages.lock().unwrap().push("Weave Loader download complete!".to_string());
                            });
                        }

                        self.lunar_weave_ready.1 = false;
                    }

                    // Downloading status
                    if self.downloading {
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            ui.label("Downloading Weave Loader...");
                            ui.spinner();
                        });
                    }
                });

                ui.separator();

                // Right panel for action buttons
                ui.vertical_centered_justified(|ui| {
                    ui.heading("Actions");
                    ui.add_space(10.0);
                    
                    if self.lunar_weave_ready.0 && self.lunar_weave_ready.1 { // Add extra space when ready
                        ui.add_space(15.0);
                    }

                    // Load button
                    let button = egui::Button::new("Load Weave")
                        .fill(if self.lunar_weave_ready.0 && self.lunar_weave_ready.1 {
                            if ctx.style().visuals.dark_mode {
                                egui::Color32::from_rgb(0, 150, 136) // Color for dark mode
                            } else {
                                egui::Color32::from_rgb(201, 241, 226) // Color for light mode
                            }
                            
                        } else if ctx.style().visuals.dark_mode {
                            egui::Color32::DARK_GRAY // Color for dark mode
                        } else {
                            egui::Color32::LIGHT_GRAY // Color for light mode
                        })
                        .min_size((0.0, 50.0).into());

                    if ui.add_enabled(self.lunar_weave_ready.0 && self.lunar_weave_ready.1, button).clicked() {
                        let lunar_client_cloned = self.lunar_client.clone().unwrap();
                        if !lunar_client_cloned.weave_installed {
                            let log_messages = self.log_messages.clone();
                            tokio::spawn(async move {
                                launcher::launch(lunar_client_cloned, &log_messages).unwrap();
                            });
                        }
                    }

                    // Status message
                    if let Some(lunar_client) = &self.lunar_client {
                        if lunar_client.weave_installed {
                            ui.colored_label(egui::Color32::GREEN, "Loaded!");
                        }
                    }
                });
            });

            ui.separator();

            // Log messages area
            ui.horizontal(|ui| {
                ui.heading("Log Messages");
            });

            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for message in self.log_messages.lock().unwrap().iter() {
                        ui.label(message);
                    }
                    // Scroll to bottom
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                });
        });
    }
}

#[tokio::main]
async fn main() {
    eframe::run_native(
        "Weave Lunar Launcher",
        NativeOptions {
            viewport: ViewportBuilder::default()
                .with_inner_size(vec2(403.0, 570.0))
                .with_resizable(false)
                .with_maximize_button(false)
                .with_icon(icon_data::from_png_bytes(&include_bytes!("../icons/weave_loader.png")[..]).unwrap()),
            ..Default::default()
        },
        Box::new(|_cc| return Ok(Box::new(App::new()))),
    ).expect("Error running the app");
}

fn fetch_lunar_client(weave_path: &str) -> Option<LunarProcess> {
    // Create a System instance
    let mut system = System::new_all();

    // Refresh the system information to get the latest data
    system.refresh_all();

    // Iterate over all processes and find the Lunar Client process
    for (pid, process) in system.processes() {
        if process.exe().is_none() {
            continue;
        }

        let process_exe_str = process.exe().unwrap().to_str().unwrap();
        if process_exe_str.contains("bin/java") && process_exe_str.contains(".lunarclient") { // Check if the process is Lunar Client
            // Modify launch_args, where `-Dichor.filteredGenesisSentries` is removed and `-javaagent:<weave_path>` is added
            let mut launch_args_modified: Vec<String> = process.cmd()
                .iter()
                .filter(|arg| return !arg.starts_with("-Dichor.filteredGenesisSentries"))
                .map(|arg| return arg.to_string())
                .collect();
            let agent = format!("-javaagent:{weave_path}");
            launch_args_modified.insert(1, agent);
            launch_args_modified.remove(0); // Remove the first argument which is the process executable

            // Get the home path of Lunar Client, this is done by splitting the process executable's path by ".lunarclient" and taking the first part and adding back the ".lunarclient/"
            let mut lunar_client_home_path = PathBuf::from(format!("{}{}", process_exe_str.splitn(2, ".lunarclient").collect::<Vec<&str>>()[0], ".lunarclient"));
            lunar_client_home_path.push("offline"); // Go into the offline directory
            lunar_client_home_path.push("multiver"); // Go into the multiver directory

            return Some(LunarProcess {
                pid: pid.as_u32(),
                exe: process_exe_str.to_string(),
                weave_installed: process.cmd().contains(&format!("-javaagent:{weave_path}")),
                launch_cmd_modified: launch_args_modified,
                flatpak: !Path::new(&lunar_client_home_path).exists(), // If the home path does not exist, it is a Flatpak installation
                home_path: lunar_client_home_path
            });
        }
    }
    return None;
}

// Returns a tuple containing a boolean indicating if the Weave Loader is installed and the path to the expected Weave Loader
fn get_weave_loader() -> (bool, PathBuf) {
    let home_dir = home_dir().unwrap();
    let weave_home = home_dir.join(".weave");

    if !weave_home.exists() {
        // Create the .weave directory
        fs::create_dir(&weave_home).unwrap();
    }

    let weave_loader = weave_home.join("loader.jar");
    if !weave_loader.exists() {
        return (false, weave_loader);
    }
    return (true, weave_loader);
}
