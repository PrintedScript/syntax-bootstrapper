use chrono::format;
use colored::*;
use std::io::Write;
use std::path::PathBuf;
use reqwest::Client;
use dirs::data_local_dir;
use futures_util::StreamExt;
use md5;
use zip_extract;

#[cfg(target_os = "windows")]
use std::os::windows::prelude::FileExt;
#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;
#[cfg(not(target_os = "windows"))]
use std::io::prelude::*;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::FileExt;

pub mod log;

include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

const ASCII_ART: &str = include_str!(concat!(env!("OUT_DIR"), "./ascii.txt"));
const SMALL_ACII: &str = include_str!(concat!(env!("OUT_DIR"), "./ascii_small.txt"));

pub async fn http_get(client: &Client, url: &str) -> Result<String, reqwest::Error> {
    debug!("{} {}", "GET".green(), url.bright_blue());
    let response = client.get(url).send().await;
    if response.is_err() {
        debug!("Failed to fetch {}", url.bright_blue());
        return Err(response.err().unwrap());
    }
    let response_body = response.unwrap().text().await.unwrap();
    Ok(response_body)
}

pub async fn download_file(client: &Client, url: &str, path: &PathBuf) {
    debug!("{} {}", "GET".green(), url.bright_blue());
    let response = client.get(url).send().await.unwrap();
    let content_length = response.content_length().unwrap();
    debug!("Content Length: {}", content_length);

    let time = chrono::Local::now().format("%H:%M:%S").to_string();
    let pg_bar_str = "                ";
    let progress_bar = indicatif::ProgressBar::new(content_length);

    // progress_bar.set_style(progress_style);
    progress_bar.set_message("Downloading File");

    let mut file = std::fs::File::create(path).unwrap();
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.or(Err(format!("Error while downloading file"))).unwrap();
        file.write_all(&chunk);
        let new = std::cmp::min(downloaded + (chunk.len() as u64), content_length);
        downloaded = new;
        progress_bar.set_position(new);
    }
    progress_bar.finish();
    info!("Finished downloading {}", url.green());
}

pub async fn download_file_prefix(client: &Client, url: &str, path_prefix: &PathBuf) -> PathBuf {
    let path = path_prefix.join(generate_md5(url).await);
    download_file(client, url, &path).await;
    return path;
}

pub async fn generate_md5(input: &str) -> String {
    let hashed_input = md5::compute(input.as_bytes());
    return format!("{:x}", hashed_input);
}

pub async fn create_folder_if_not_exists(path: &PathBuf) {
    if !path.exists() {
        info!("Creating folder {}", path.to_str().unwrap().bright_blue());
        std::fs::create_dir_all(path).unwrap();
    }
}

fn get_installation_directory() -> PathBuf {
    return PathBuf::from(data_local_dir().unwrap().to_str().unwrap()).join("Syntax");
}

fn format_info_line<T: AsRef<str>>(data: T) -> ColoredString {
    let data = data.as_ref();

    data.magenta().cyan().italic().on_black()
}

fn is_large() -> bool {
    if let Some((terminal_width, _)) = term_size::dimensions() {
        return terminal_width > 80;
    } else {
        return false;
    };
}

fn startup_info() {
    /* Clear screen */
    print!("\x1b[2J\x1b[H");

    if !is_large() {
        return println!("{}", format_info_line(SMALL_ACII));
    }

    let mut lines: Vec<&str> = ASCII_ART.lines().collect();
    let last = lines.pop();

    for value in lines {
        println!("{}", value.bright_magenta().italic().on_black());
    }
    if let Some(last_val) = last {
        println!("{}", format_info_line(last_val));
    }
}

fn main() {
    /* Ansi character to clear line */

    startup_info();

    log::info!("{}", SETUP_URL);
    log::debug!("Hello {}", "World");
    log::error!("Oh no there was an issue");
    log::fatal!("Oh shit")
}
