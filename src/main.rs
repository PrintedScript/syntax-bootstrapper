use colored::*;
use dirs::data_local_dir;
use futures_util::StreamExt;
use md5;
use metadata::LevelFilter;
use reqwest::Client;
use std::path::PathBuf;
use tokio::fs::create_dir_all;
use tokio::task::JoinSet;
use zip_extract;

mod constants;
mod util;

use constants::*;

use tracing::*;
use util::*;

#[cfg(not(target_os = "windows"))]
use std::io::prelude::*;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::FileExt;
#[cfg(target_os = "windows")]
use std::os::windows::prelude::FileExt;
#[cfg(target_os = "windows")]
use winreg::enums::*;
#[cfg(target_os = "windows")]
use winreg::RegKey;

#[cfg(debug_assertions)]
const DEBUG: bool = true;
#[cfg(debug_assertions)]
const MAX_TRACING_LEVEL: LevelFilter = LevelFilter::DEBUG;
#[cfg(not(debug_assertions))]
const DEBUG: bool = false;
#[cfg(not(debug_assertions))]
const MAX_TRACING_LEVEL: LevelFilter = LevelFilter::ERROR;

const ASCII_ART: &str = include_str!("./ascii.txt");

#[tokio::main]
async fn main() {
    // Clear the terminal before printing the startup text
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/c", "cls"])
            .spawn()
            .expect("cls command failed to start")
            .wait()
            .expect("failed to wait");
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("clear").spawn().unwrap();
    }

    let args: Vec<String> = std::env::args().collect();
    let base_url: &str = "www.syntax.eco";
    let mut setup_url: &str = "setup.syntax.eco";
    let fallback_setup_url: &str = "d2f3pa9j0u8v6f.cloudfront.net";
    let mut bootstrapper_filename: &str = "SyntaxPlayerLauncher.exe";
    #[cfg(not(target_os = "windows"))]
    {
        bootstrapper_filename = "SyntaxPlayerLinuxLauncher";
    }
    let build_date = include_str!(concat!(env!("OUT_DIR"), "/build_date.txt"));
    let startup_text = ASCII_ART.to_owned();

    let bootstrapper_info = format!(
        "{} | Build Date: {} | Version: {}",
        base_url,
        build_date,
        env!("CARGO_PKG_VERSION"),
    );

    // Format the startup text to be centered
    let mut terminal_width = 80;
    if let Some((w, _h)) = term_size::dimensions() {
        terminal_width = w;
    }
    if terminal_width < 80 {
        print!(
            "{}\n",
            format!(
                "SYNTAX Bootstrapper | {} | Build Date: {} | Version: {}",
                base_url,
                build_date,
                env!("CARGO_PKG_VERSION")
            )
            .to_string()
            .magenta()
            .cyan()
            .italic()
            .on_black()
        ); // Fallback message
    } else {
        let startup_text_lines = startup_text.lines().collect::<Vec<&str>>();
        //println!("{}", startup_text.bold().blue().on_black());

        // print all lines except the last one
        for line in startup_text_lines {
            let spaces = (terminal_width - line.len()) / 2;
            let formatted_line = format!("{}{}", " ".repeat(spaces), line);
            println!("{}", formatted_line.bright_magenta().italic().on_black());
        }

        // print last line as a different color
        println!(
            "{}\n",
            bootstrapper_info.magenta().cyan().italic().on_black()
        );
    }

    tracing_subscriber::fmt()
        .with_max_level(MAX_TRACING_LEVEL)
        .pretty()
        .init();

    let http_client: Client = reqwest::Client::builder().no_gzip().build().unwrap();
    debug!(
        "Setup Server: {} | Base Server: {}",
        setup_url.bright_blue(),
        base_url.bright_blue()
    );

    debug!("Fetching latest client version from setup server");

    let latest_client_version: String;
    let latest_client_version_response =
        http_get(&http_client, &format!("https://{}/version", setup_url)).await;
    match latest_client_version_response {
        Ok(latest_client_version_result) => {
            debug!(
                "Latest Client Version: {}",
                latest_client_version_result.bright_blue()
            );
            latest_client_version = latest_client_version_result;
        }
        Err(e) => {
            error!("Failed to fetch latest client version from setup server: [{}], attempting to fallback to {}", e.to_string().bright_red(), fallback_setup_url.bright_blue());
            let fallback_client_version_response = http_get(
                &http_client,
                &format!("https://{}/version", fallback_setup_url),
            )
            .await;
            match fallback_client_version_response {
                Ok(fallback_client_version_result) => {
                    info!(
                        "Successfully fetched latest client version from fallback setup server: {}",
                        fallback_setup_url.bright_blue()
                    );
                    debug!(
                        "Latest Client Version: {}",
                        fallback_client_version_result.bright_blue()
                    );
                    latest_client_version = fallback_client_version_result;
                    setup_url = fallback_setup_url;
                }
                Err(e) => {
                    error!("Failed to fetch latest client version from fallback setup server: {}, are you connected to the internet?", e);
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    std::process::exit(0);
                }
            }
        }
    }

    // Wait for the latest client version to be fetched
    info!(
        "Latest Client Version: {}",
        latest_client_version.cyan().underline()
    );
    debug!("Setup Server: {}", setup_url.cyan().underline());

    let installation_directory = get_installation_directory();
    debug!(
        "Instillation Directory: {}",
        format!("{:?}", installation_directory.display()).bright_blue()
    );
    create_folder_if_not_exists(&installation_directory).await;

    let versions_directory = installation_directory.join("Versions");
    debug!(
        "Versions Directory: {}",
        format!("{:?}", versions_directory.display()).bright_blue()
    );
    create_folder_if_not_exists(&versions_directory).await;

    let temp_downloads_directory = installation_directory.join("Downloads");
    debug!(
        "Temp Downloads Directory: {}",
        format!("{:?}", temp_downloads_directory.display()).bright_blue()
    );
    create_folder_if_not_exists(&temp_downloads_directory).await;

    let current_version_directory = versions_directory.join(format!("{}", latest_client_version));
    debug!(
        "Current Version Directory: {}",
        format!("{:?}", current_version_directory.display()).bright_blue()
    );

    create_folder_if_not_exists(&current_version_directory).await;

    let latest_bootstrapper_path = current_version_directory.join(bootstrapper_filename);
    // Is the program currently running from the latest version directory?
    let current_exe_path = std::env::current_exe().unwrap();
    // If the current exe path is not in the current version directory, then we need to run the latest bootstrapper ( download if needed )
    if !current_exe_path.starts_with(&current_version_directory) && !DEBUG {
        // Check if the latest bootstrapper is downloaded
        if !latest_bootstrapper_path.exists() {
            info!("Downloading the latest bootstrapper and restarting");
            // Download the latest bootstrapper
            download_to_file(
                &http_client,
                &format!(
                    "https://{}/{}-{}",
                    setup_url, latest_client_version, bootstrapper_filename
                ),
                &latest_bootstrapper_path,
            )
            .await;
        }
        // Run the latest bootstrapper ( with the same arguments passed to us ) and exit
        #[cfg(target_os = "windows")]
        {
            let mut command = std::process::Command::new(latest_bootstrapper_path.clone());
            command.args(&args[1..]);
            match command.spawn() {
                Ok(_) => {}
                Err(e) => {
                    debug!("Bootstrapper errored with error {}", e);
                    info!("Found bootstrapper was corrupted! Downloading...");
                    std::fs::remove_file(latest_bootstrapper_path.clone()).unwrap();
                    download_to_file(
                        &http_client,
                        &format!(
                            "https://{}/{}-{}",
                            setup_url, latest_client_version, bootstrapper_filename
                        ),
                        &latest_bootstrapper_path,
                    )
                    .await;
                    command.spawn().expect("Bootstrapper is still corrupted.");
                    std::thread::sleep(std::time::Duration::from_secs(20));
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            // Make sure the latest bootstrapper is executable
            std::process::Command::new("chmod")
                .arg("+x")
                .arg(latest_bootstrapper_path.to_str().unwrap())
                .spawn()
                .unwrap();

            info!("We need permission to run the latest bootstrapper");
            let mut command = std::process::Command::new(latest_bootstrapper_path);
            command.args(&args[1..]);
            command.spawn().unwrap();
        }
        std::process::exit(0);
    }

    // Looks like we are running from the latest version directory, so we can continue with the update process
    // Check for "AppSettings.xml" in the current version directory
    // If it doesent exist, then we got either a fresh directory or a corrupted installation
    // So delete the every file in the current version directory except for the Bootstrapper itself
    let app_settings_path = current_version_directory.join("AppSettings.xml");
    let client_executable_path = current_version_directory.join("SyntaxPlayerBeta.exe");
    if !app_settings_path.exists() || !client_executable_path.exists() {
        info!("Downloading the latest client files, this may take a while.");
        for entry in std::fs::read_dir(&current_version_directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                if path != current_exe_path {
                    std::fs::remove_file(path).unwrap();
                }
            } else {
                std::fs::remove_dir_all(path).unwrap();
            }
        }

        let version_url_prefix = format!("https://{}/{}-", setup_url, latest_client_version);

        /* Use a joisnet to run multiple async functions at once */

        let mut set = JoinSet::new();

        for [value, path] in FILES_TO_DOWNLOAD {
            create_dir_all(current_version_directory.join(path))
                .await
                .unwrap();
            set.spawn(download_and_extract(
                value.to_string(),
                version_url_prefix.clone(),
                current_version_directory.clone(),
            ));
        }

        while let Some(value) = set.join_next().await {
            value.unwrap()
        }

        info!("Binary installed");

        /* Convert to async due to this being a slow function */

        // Redacted for lagging vscode

        // Install the syntax-player scheme in the registry
        info!("Installing syntax-player scheme");
        #[cfg(target_os = "windows")]
        {
            let hkey_current_user = RegKey::predef(HKEY_CURRENT_USER);
            let hkey_classes_root: RegKey =
                hkey_current_user.open_subkey("Software\\Classes").unwrap();
            let hkey_syntax_player = hkey_classes_root.create_subkey("syntax-player").unwrap().0;
            let hkey_syntax_player_shell = hkey_syntax_player.create_subkey("shell").unwrap().0;
            let hkey_syntax_player_shell_open =
                hkey_syntax_player_shell.create_subkey("open").unwrap().0;
            let hkey_syntax_player_shell_open_command = hkey_syntax_player_shell_open
                .create_subkey("command")
                .unwrap()
                .0;
            let defaulticon = hkey_syntax_player.create_subkey("DefaultIcon").unwrap().0;
            hkey_syntax_player_shell_open_command
                .set_value(
                    "",
                    &format!("\"{}\" \"%1\"", current_exe_path.to_str().unwrap()),
                )
                .unwrap();
            defaulticon
                .set_value("", &format!("\"{}\",0", current_exe_path.to_str().unwrap()))
                .unwrap();
            hkey_syntax_player
                .set_value("", &format!("URL: Syntax Protocol"))
                .unwrap();
            hkey_syntax_player.set_value("URL Protocol", &"").unwrap();
        }
        #[cfg(not(target_os = "windows"))]
        {
            // Linux support
            // We have to write a .desktop file to ~/.local/share/applications
            let desktop_file_path = dirs::data_local_dir()
                .unwrap()
                .join("applications")
                .join("syntax-player.desktop");
            let desktop_file = format!(
                "[Desktop Entry]
Name=Syntax Launcher
Exec={} %u
Terminal=true
Type=Application
MimeType=x-scheme-handler/syntax-player;
Icon={}
StartupWMClass=SyntaxLauncher
Categories=Game;
Comment=Syntax Launcher
",
                current_exe_path.to_str().unwrap(),
                current_exe_path.to_str().unwrap()
            );
            std::fs::write(desktop_file_path, desktop_file).unwrap();
            // We also have to write a mimeapps.list file to ~/.config
            let mimeapps_list_path = dirs::config_dir().unwrap().join("mimeapps.list");
            let mimeapps_list = format!(
                "[Default Applications]
x-scheme-handler/syntax-player=syntax-player.desktop
"
            );
            std::fs::write(mimeapps_list_path, mimeapps_list).unwrap();
            // We also have to write a mimeapps.list file to ~/.local/share
            let mimeapps_list_path = dirs::data_local_dir().unwrap().join("mimeapps.list");
            let mimeapps_list = format!(
                "[Default Applications]
x-scheme-handler/syntax-player=syntax-player.desktop
"
            );
            std::fs::write(mimeapps_list_path, mimeapps_list).unwrap();
        }

        // Write the AppSettings.xml file
        let app_settings_xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<Settings>
	<ContentFolder>content</ContentFolder>
	<BaseUrl>https://{}</BaseUrl>
</Settings>",
            base_url
        );
        std::fs::write(app_settings_path, app_settings_xml).unwrap();

        // Check for any other version directories and deletes them
        for entry in std::fs::read_dir(&versions_directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                if path != current_version_directory {
                    std::fs::remove_dir_all(path).unwrap();
                }
            }
        }
    }

    // Parse the arguments passed to the bootstrapper
    // Looks something like "syntax-player://1+launchmode:play+gameinfo:TICKET+placelauncherurl:https://www.syntax.eco/Game/placelauncher.ashx?placeId=660&t=TICKET+k:l"
    debug!("Arguments Passed: {}", args.join(" ").bright_blue());
    if args.len() == 1 {
        // Just open the website
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .arg("/c")
                .arg("start")
                .arg("https://www.syntax.eco/games")
                .spawn()
                .unwrap();
            std::process::exit(0);
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new("xdg-open")
                .arg("https://www.syntax.eco/games")
                .spawn()
                .unwrap();
            std::process::exit(0);
        }
    }

    let main_args = &args[1];
    let main_args = main_args.replace("syntax-player://", "");
    let main_args = main_args.split("+").collect::<Vec<&str>>();

    let mut launch_mode = String::new();
    let mut authentication_ticket = String::new();
    let mut join_script = String::new();
    let mut client_year = String::new();

    for arg in main_args {
        let mut arg_split = arg.split(":");
        let key = arg_split.next().unwrap();
        let value = if arg_split.clone().count() > 0 {
            arg_split.collect::<Vec<&str>>().join(":")
        } else {
            String::new()
        };
        debug!("{}: {}", key.bright_blue(), value.bright_blue());
        match key {
            "launchmode" => {
                launch_mode = value.to_string();
            }
            "gameinfo" => {
                authentication_ticket = value.to_string();
            }
            "placelauncherurl" => {
                join_script = value.to_string();
            }
            "clientyear" => {
                client_year = value.to_string();
            }
            _ => {}
        }
    }

    let custom_wine = "wine";
    #[cfg(not(target_os = "windows"))]
    {
        // We allow user to specify the wine binary path in installation_directory/winepath.txt
        let wine_path_file = installation_directory.join("winepath.txt");
        if wine_path_file.exists() {
            let custom_wine = std::fs::read_to_string(wine_path_file).unwrap();
            info!("Using custom wine binary: {}", custom_wine.bright_blue());
        } else {
            info!("No custom wine binary specified, using default wine command");
            info!("If you want to use a custom wine binary, please create a file at {} with the path to the wine binary", wine_path_file.to_str().unwrap());
        }
    }
    let client_executable_path: PathBuf;
    debug!("{}", &client_year.to_string());
    if client_year == "2018" {
        client_executable_path = current_version_directory
            .join("Client2018")
            .join("SyntaxPlayerBeta.exe");
    } else if client_year == "2020" {
        client_executable_path = current_version_directory
            .join("Client2020")
            .join("SyntaxPlayerBeta.exe");
    } else {
        client_executable_path = current_version_directory.join("SyntaxPlayerBeta.exe");
    }
    if !client_executable_path.exists() {
        // Delete AppSettings.xml so the bootstrapper will download the client again
        let app_settings_path = current_version_directory.join("AppSettings.xml");
        std::fs::remove_file(app_settings_path).unwrap();

        error!("Failed to run SyntaxPlayerBeta.exe, is your antivirus removing it? The bootstrapper will attempt to redownload the client on next launch.");
        std::thread::sleep(std::time::Duration::from_secs(20));
        std::process::exit(0);
    }
    match launch_mode.as_str() {
        "play" => {
            info!("Launching SYNTAX");
            #[cfg(target_os = "windows")]
            {
                let mut command = std::process::Command::new(client_executable_path);
                command.args(&[
                    "--play",
                    "--authenticationUrl",
                    format!("https://{}/Login/Negotiate.ashx", base_url).as_str(),
                    "--authenticationTicket",
                    authentication_ticket.as_str(),
                    "--joinScriptUrl",
                    format!("{}", join_script.as_str()).as_str(),
                ]);
                command.spawn().unwrap();
                std::thread::sleep(std::time::Duration::from_secs(5));
                std::process::exit(0);
            }
            #[cfg(not(target_os = "windows"))]
            {
                // We have to launch the game through wine
                let mut command = std::process::Command::new(custom_wine);
                command.args(&[
                    client_executable_path.to_str().unwrap(),
                    "--play",
                    "--authenticationUrl",
                    format!("https://{}/Login/Negotiate.ashx", base_url).as_str(),
                    "--authenticationTicket",
                    authentication_ticket.as_str(),
                    "--joinScriptUrl",
                    format!("{}", join_script.as_str()).as_str(),
                ]);
                // We must wait for the game to exit before exiting the bootstrapper
                let mut child = command.spawn().unwrap();
                child.wait().unwrap();
                std::thread::sleep(std::time::Duration::from_secs(1));
                std::process::exit(0);
            }
        }
        _ => {
            error!("Unknown launch mode, exiting.");
            std::thread::sleep(std::time::Duration::from_secs(10));
            std::process::exit(0);
        }
    }
}
