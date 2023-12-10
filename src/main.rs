use chrono::{DateTime, Local};
use log::{error, info};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const BUFFER_FILE_PATH: &str = ".polybar-internet-speed.toml";

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

fn get_buffered_filename() -> Result<String, String> {
    let xdg = match env::var("XDG_CACHE_HOME") {
        Ok(x) => x,
        Err(e) => {
            return Err(format!("Failed to get XDG_CACHE_HOME: {}", e));
        }
    };
    let path = PathBuf::from(xdg).join(BUFFER_FILE_PATH);
    let file = match path.to_str() {
        Some(f) => f,
        None => {
            return Err(format!("Failed to convert path to string"));
        }
    };
    Ok(file.to_string())
}

fn get_new_internet_info() -> Result<Fast, String> {
    let info = match get_internet_info() {
        Ok(f) => f,
        Err(e) => {
            return Err(format!("{}", e));
        }
    };
    let path = match get_buffered_filename() {
        Ok(p) => p,
        Err(e) => {
            return Err(format!("{}", e));
        }
    };
    match write_buffered_file(&path, &info) {
        Ok(_) => (),
        Err(e) => {
            return Err(format!("{}", e));
        }
    }
    Ok(info)
}

fn get_buffered_internet_info() -> Result<Fast, String> {
    let path = match get_buffered_filename() {
        Ok(p) => p,
        Err(e) => {
            return Err(format!("{}", e));
        }
    };
    let file_contents = match fs::read_to_string(&path) {
        Ok(f) => f,
        Err(e) => {
            return Err(format!("Failed to read file: {}", e));
        }
    };
    let info = match toml::from_str(&file_contents) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to parse TOML: {}", e);
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
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} [{t} {l} {M}:{L}] - {m}{n}",
        )))
        .build("/tmp/polybar-internet-speed.log")
        .unwrap();
    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("logfile")
                .build(log::LevelFilter::Info),
        )
        .unwrap();
    let _handle = log4rs::init_config(config).unwrap();
    let path = match get_buffered_filename() {
        Ok(p) => p,
        Err(e) => {
            error!("{}", e);
            return;
        }
    };
    // Check if there's an up to date buffered file
    let info = match get_seconds_since_file_modified(&path) {
        Ok(elapsed) => match elapsed {
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
        },
        Err(e) => {
            info!("Buffered file doesn't exist");
            let info = match get_new_internet_info() {
                Ok(i) => i,
                Err(e2) => {
                    error!(
                        "File didn't exist: Error: {}. Tried to create it: Error: {}",
                        e, e2
                    );
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
