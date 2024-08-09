use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::spawn;

use sysinfo::System;

use crate::LunarProcess;

pub fn launch(lunar_process: LunarProcess, log_messages: &Arc<Mutex<Vec<String>>>) -> Result<(), Box<dyn std::error::Error>> {
    // Kill the Lunar Client process
    kill_process(lunar_process.pid)?;

    // Handle things differently on Flatpak
    if lunar_process.flatpak {
        launch_flatpak(lunar_process, log_messages)?;
        return Ok(());
    }

    // Launch the Lunar Client process
    let command = Command::new(&lunar_process.exe)
        .current_dir(&lunar_process.home_path)
        .args(&lunar_process.launch_cmd_modified)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .envs(get_gpu_env_vars()) // Set the GPU environment variables to make it use the dedicated GPU on Linux
        .spawn()?;

    handle_output(command, log_messages);

    return Ok(())
}

fn launch_flatpak(lunar_process: LunarProcess, log_messages: &Arc<Mutex<Vec<String>>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = vec!["cd".to_string(), lunar_process.home_path.to_str().unwrap().to_string(), "&&".to_string(), lunar_process.exe];
    args.extend(lunar_process.launch_cmd_modified);

    return run_through_flatpak(&args, log_messages)
}

fn run_through_flatpak(args: &[String], log_messages: &Arc<Mutex<Vec<String>>>) -> Result<(), Box<dyn std::error::Error>> {
    // Get the app ID
    let output = Command::new("flatpak")
        .args(["list", "--app"])
        .output()?;

    let app_list = String::from_utf8(output.stdout)?;
    let app_id = app_list
        .lines()
        .find(|line| return line.to_lowercase().contains("lunar"))
        .and_then(|line| return line.split_whitespace().nth(1))
        .ok_or("Lunar Client not found in Flatpak list")?;

    println!("Running LunarClient through Flatpak with app ID: {app_id}");
    println!("Arguments: {args:?}");

    // Prepare the command
    let mut flatpak_args = vec!["run", "--command=sh", app_id, "-c"];
    let cmd = args.join(" ");
    flatpak_args.push(&cmd);

    // Run the command through Flatpak
    let command = Command::new("flatpak")
        .args(&flatpak_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    handle_output(command, log_messages);

    return Ok(())
}

fn handle_output(mut command: Child, log_messages: &Arc<Mutex<Vec<String>>>) {
    let stdout = command.stdout.take().unwrap();
    let stderr = command.stderr.take().unwrap();

    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);

    let log_messages_clone = log_messages.clone();
    let stdout_thread = spawn(move || {
        for line in stdout_reader.lines().map_while(Result::ok) {
            log_messages_clone.lock().unwrap().push(line);
        }
    });

    let log_messages_clone = log_messages.clone();
    let stderr_thread = spawn(move || {
        for line in stderr_reader.lines().map_while(Result::ok) {
            log_messages_clone.lock().unwrap().push(format!("ERR: {line}"));
        }
    });

    stdout_thread.join().unwrap();
    stderr_thread.join().unwrap();
}

fn kill_process(pid: u32) -> Result<(), String> {
    let mut system = System::new_all();
    system.refresh_all();

    let pid = sysinfo::Pid::from(pid as usize);

    return if let Some(process) = system.process(pid) {
        if process.kill() {
            Ok(())
        } else {
            Err(format!("Failed to kill process with PID {pid}"))
        }
    } else {
        Err(format!("Process with PID {pid} not found"))
    }
}

fn get_gpu_env_vars() -> Vec<(String, String)> {
    // Check if it's Windows or macOS
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    return Vec::new();

    // For other operating systems (assuming Linux)
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Check if two GPUs exist
        let lspci_output = Command::new("lspci")
            .arg("-nn")
            .output()
            .expect("Failed to execute lspci command");

        let output = String::from_utf8_lossy(&lspci_output.stdout);
        let gpu_count = output.matches("VGA compatible controller").count();

        if gpu_count < 2 {
            return Vec::new();
        }

        // Check if the second GPU is NVIDIA or AMD
        if output.contains("NVIDIA") {
            return vec![
                (String::from("__NV_PRIME_RENDER_OFFLOAD"), String::from("1")),
                (String::from("__GLX_VENDOR_LIBRARY_NAME"), String::from("nvidia")),
            ]
        } else if output.contains("Advanced Micro Devices") || output.contains("AMD") {
            return vec![
                (String::from("DRI_PRIME"), String::from("1")),
            ]
        }
        
        return Vec::new()
    }
}