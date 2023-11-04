/*
    Move all generic functions into this area
*/
use colored::*;
use dirs::data_local_dir;
use futures_util::StreamExt;
use reqwest::Client;
use reqwest::ClientBuilder;
use std::io::Cursor;
use std::path::PathBuf;
use tokio::fs;
use tokio::fs::create_dir_all;
use zip_extract;

use crate::constants::*;
use tracing::*;

use std::io::prelude::*;
/*
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::FileExt;
#[cfg(target_os = "windows")]
use std::os::windows::prelude::FileExt;
*/
#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

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

pub async fn download_file(client: &Client, url: &str) -> Vec<u8> {
    debug!("{} {}", "GET".green(), url.bright_blue());
    let response = client.get(url).send().await.unwrap();
    /* Why through over a visual bug? */
    let content_length = response.content_length().or(Some(0)).unwrap();
    debug!("Content Length: {}", content_length);

    let time = chrono::Local::now().format("%H:%M:%S").to_string();
    let pg_bar_str =
        "                {spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})";
    let progress_bar = indicatif::ProgressBar::new(content_length);
    let progress_style = indicatif::ProgressStyle::default_bar()
        .template(
            format!(
                "{}\n{}",
                format!(
                    "[{}] [{}] {}",
                    time.bold().blue(),
                    "INFO".bold().green(),
                    &format!("Downloading {}", &url.bright_blue())
                ),
                pg_bar_str
            )
            .as_str(),
        )
        .unwrap()
        .progress_chars("#>-");
    progress_bar.set_style(progress_style);
    progress_bar.set_message("Downloading File");

    let mut buffer: Vec<u8> = vec![];
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item
            .or(Err(format!("Error while downloading file")))
            .unwrap();

        buffer.write_all(chunk.as_ref()).unwrap();
        let new = std::cmp::min(downloaded + (chunk.len() as u64), content_length);
        downloaded = new;
        progress_bar.set_position(new);
    }
    progress_bar.finish();
    info!("Finished downloading {}", url.green());

    return buffer;
}

pub async fn download_to_file(client: &Client, url: &str, path: &PathBuf) {
    let bytes = download_file(client, url).await;
    fs::write(path, bytes).await.unwrap();
}

pub async fn download_file_prefix<T: Into<String>>(client: &Client, url: T) -> Vec<u8> {
    let string: String = url.into();
    let buffer = download_file(client, &string).await;
    return buffer;
}

/*
pub async fn generate_md5(input: &str) -> String {
    let hashed_input = md5::compute(input.as_bytes());
    return format!("{:x}", hashed_input);
}*/
pub async fn create_folder_if_not_exists(path: &PathBuf) {
    if !path.exists() {
        info!("Creating folder {}", path.to_str().unwrap().bright_blue());
        fs::create_dir_all(path).await.unwrap();
    }
}

pub fn get_installation_directory() -> PathBuf {
    return PathBuf::from(data_local_dir().unwrap().to_str().unwrap()).join("Syntax");
}

/* Why was this in the main function i will never know */
pub async fn extract_to_dir(zip_file: &Vec<u8>, target_dir: &PathBuf) {
    let zip_file_cursor = Cursor::new(zip_file);
    zip_extract::extract(zip_file_cursor, target_dir, false).unwrap();
}

fn get_location_from_file_name<T: AsRef<str>>(file_name: T) -> String {
    let file_name = file_name.as_ref();
    for [first, last] in FILES_TO_DOWNLOAD {
        if first == file_name {
            return last.to_owned();
        }
    }
    let formated = format!("Is not a valid file {}", file_name);
    error!("{}", formated);
    panic!("{}", formated)
}

pub async fn download_and_extract<T: Into<String>, T2: Into<String>, P: Into<PathBuf>>(
    file_name: T,
    url_prefix: T2,
    extract_location: P,
) {
    let http_client = ClientBuilder::default().build().unwrap();
    let file_name: String = file_name.into();
    let url_prefix: String = url_prefix.into();
    let extract_location: PathBuf = extract_location.into();

    let buffer = download_file_prefix(&http_client, format!("{}{file_name}", url_prefix)).await;
    drop(http_client);
    drop(url_prefix);
    let dir = extract_location.join(get_location_from_file_name(&file_name));

    create_dir_all(&dir).await.unwrap();
    info!("Extracting file {}", file_name);
    extract_to_dir(&buffer, &dir).await;
    drop(buffer);

    info!("File {} installed to {:?}", file_name, dir.display());
}
