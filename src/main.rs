use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::process::Command;

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

#[derive(Debug, Deserialize)]
struct Fast {
    downloadSpeed: u32,
    downloaded: u32,
    latency: u32,
    bufferBloat: u32,
    userLocation: String,
    userIp: String,
}

fn main() {
    // Check if there's an up to date buffered file
    const BUFFER_FILE_PATH: &str = "/tmp/flux";
    let _elap_time = match get_seconds_since_file_modified(BUFFER_FILE_PATH) {
        Ok(value) => println!("Result: {}", value),
        Err(error) => println!("Error: {}", error),
    };

    println!("Checking internet speed. Please wait...");
    let output = Command::new("fast")
        .arg("--json")
        .output()
        .expect("Failed to execute command");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("\tCommand failed:\n{}", &stderr);
        return;
    }

    let o = String::from_utf8_lossy(&output.stdout);
    // let o = r#"{ "downloadSpeed": 330, "downloaded": 310, "latency": 17, "bufferBloat": 143, "userLocation": "Clearwater, US", "userIp": "72.187.132.254" }"#;
    let f: Fast = match serde_json::from_str(&o) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to parse JSON: {}", e);
            return;
        }
    };

    println!("Download speed: {} Mbps", f.downloadSpeed);
}
