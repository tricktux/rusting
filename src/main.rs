use chrono::{DateTime, Local};
use log::{info, warn, error};
use fern::Dispatch;
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::process::Command;

const BUFFER_FILE_PATH: &str = "/tmp/internet-speed.toml";

fn get_seconds_since_file_modified(file: &str) -> Result<u64, String> {
    let fmeta = match fs::metadata(file) {
        Ok(meta) => meta,
        Err(e) => {
            return Err(format!(
                "Failed to get metadata for file: '{}'. Error: '{}'",
                file, e
            ));
        }
    };
    if !fmeta.is_file() {
        return Err(format!("'{}' is not a file!", file));
    }

    let file_age = match fmeta.modified() {
        Ok(t) => t,
        Err(e) => {
            return Err(format!(
                "Failed to get file modified time for file: '{}'. Error '{}'",
                file, e
            ));
        }
    };

    let ltime = Local::now();
    let ftime: DateTime<Local> = DateTime::from(file_age);
    let elapsed = match ltime.signed_duration_since(ftime).to_std() {
        Ok(e) => e.as_secs(),
        Err(e) => {
            return Err(format!("Failed to get elapsed time. Error '{}'", e));
        }
    };
    Ok(elapsed)
}

fn get_internet_info() -> Result<Fast, String> {
    // println!("Checking internet speed. Please wait...");
    let output = Command::new("fast")
        .arg("--json")
        .output()
        .expect("Failed to execute command");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("\tCommand failed:\n{}", &stderr));
    }

    let o = String::from_utf8_lossy(&output.stdout);
    info!("Command output: {}", o);
    let f: Fast = match serde_json::from_str(&o) {
        Ok(f) => f,
        Err(e) => {
            return Err(format!("Failed to parse JSON: {}", e));
        }
    };

    Ok(f)
}

fn write_buffered_file(file: &str, info: &Fast) -> Result<(), String> {
    let toml = match toml::to_string(&info) {
        Ok(t) => t,
        Err(e) => {
            return Err(format!("Failed to convert to TOML: {}", e));
        }
    };
    match fs::write(file, toml) {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("Failed to write buffered file: {}", e));
        }
    }
    Ok(())
}

fn get_new_internet_info() -> Result<Fast, String> {
    let info = match get_internet_info() {
        Ok(f) => f,
        Err(e) => {
            return Err(format!("{}", e));
        }
    };
    match write_buffered_file(BUFFER_FILE_PATH, &info) {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("{}", e));
        }
    }
    Ok(info)
}

fn get_buffered_internet_info() -> Result<Fast, String> {
    let file = match fs::read_to_string(BUFFER_FILE_PATH) {
        Ok(f) => f,
        Err(e) => {
            return Err(format!("{}", e));
        }
    };
    let info = match toml::from_str(&file) {
        Ok(f) => f,
        Err(e) => {
            return Err(format!("{}", e));
        }
    };
    Ok(info)
}

#[derive(Debug, Deserialize, Serialize)]
struct Fast {
    downloadSpeed: u32,
    latency: u32,
}

fn main() {
    Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] {} - {}",
                record.level(),
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                message
            ))
        })
        .chain(fern::log_file("/tmp/output.log").expect("Failed to open log file"))
        .apply()
        .expect("Failed to initialize logger");
    // Check if there's an up to date buffered file
    let info = match get_seconds_since_file_modified(BUFFER_FILE_PATH) {
        Ok(elapsed) => {
            match elapsed {
                0..=86400 => {
                    info!("Using buffered file: elapse = {}", elapsed);
                    let info = match get_buffered_internet_info() {
                        Ok(f) => f,
                        Err(e) => {
                            error!("{}", e);
                            return;
                        }
                    };
                    info
                }
                _ => {
                    info!("Buffered file is out of date");
                    let info = match get_new_internet_info() {
                        Ok(f) => f,
                        Err(e) => {
                            error!("{}", e);
                            return;
                        }
                    };
                    info
                }
            }
        }
        Err(e) => {
            info!("Buffered file doesn't exist");
            let info = match get_new_internet_info() {
                Ok(i) => i,
                Err(e2) => {
                    error!("File didn't exist: Error: {}. Tried to create it: Error: {}", e, e2);
                    return;
                }
            };
            info
        }
    };

    let icon = match info.latency {
        0..=50 => r#"%{F#3cb703}%{F-}"#,
        51..=150 => r#"%{F#f9dd04}%{F-}"#,
        _ => r#"%{F#d60606}%{F-}"#,
    };
    

    println!("{icon} {} ms  {} Mbps", info.latency, info.downloadSpeed);
}
