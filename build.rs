use std::collections::HashMap;
use std::fs;
use std::env;
use std::path::PathBuf;
use winres::WindowsResource;
use serde::Deserialize;

/// CHANGES
/// [+] Use rust CFG for build  (not sure)
/// [+] A build config
/// [+] Ascii is now premade due to the result always being static anyways
/// [+] Cut down into functions

const ENGLISH_LANGUAGE: u16 = 0x0409;

#[derive(Deserialize)]
struct BuildConfig {
    #[serde(rename = "Ascii")]
    pub ascii: AsciiConfig,
    #[serde(rename = "Runtime")]
    pub runtime: RuntimeConfig,
    #[serde(rename = "Windows")]
    pub windows: WindowsConfig,
}

#[derive(Deserialize)]
struct AsciiConfig {
    #[serde(rename = "Location")]
    pub location: String,
    #[serde(rename = "Padding")]
    pub padding: usize,
    #[serde(rename = "FallbackText")]
    pub fallback_text: String,
}

#[derive(Deserialize)]
struct RuntimeConfig {
    #[serde(rename = "SetupUrl")]
    pub setup_url: String,
    #[serde(rename = "BaseUrl")]
    pub base_url: String,
    #[serde(rename = "ThreadCount")]
    pub thread_count: usize,
}

#[derive(Deserialize)]
struct WindowsConfig {
    #[serde(rename = "Icon")]
    pub icon: String,
    #[serde(rename = "Resources")]
    pub resources: HashMap<String, String>,
}

fn get_padding(size: usize) -> String {
    " ".repeat(size)
}

fn out_dir() -> PathBuf {
    let out_dir = env::var("OUT_DIR").unwrap();
    return PathBuf::from(out_dir);
}

/* Builds ascii that is displayed at startup */
fn build_large_ascii(config: &BuildConfig) {
    println!("cargo:rerun-if-changed={}", config.ascii.location);

    let build_date = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let ascii_b = fs::read(&config.ascii.location).unwrap();
    let padding = get_padding(config.ascii.padding);
    let mut ascii = String::from_utf8(ascii_b).unwrap();

    ascii += "\n";
    ascii += &format!(
        "{} | Build Date: {} | Version: {}",
        config.runtime.base_url,
        build_date,
        env!("CARGO_PKG_VERSION")
    );

    let mut new_asci = String::new();
    for line in ascii.lines() {
        new_asci += &format!("{}{}\n", padding.clone(), line);
    }
    fs::write(out_dir().join("./ascii.txt"), new_asci).unwrap();
}

fn build_small_ascii(config: &BuildConfig) {
    let build_date = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string();
    fs::write(
        out_dir().join("./ascii_small.txt"),
        format!(
            "{} | {} | Build Date: {} | Version: {}",
            config.ascii.fallback_text,
            config.runtime.base_url,
            build_date,
            env!("CARGO_PKG_VERSION")
        )
    ).unwrap()
}

fn build_ascii(config: &BuildConfig) {
    build_large_ascii(config);
    build_small_ascii(config);
}

fn build_windows(cfg: &WindowsConfig) {
    println!("cargo:rerun-if-changed={}", cfg.icon);

    let mut res = WindowsResource::new();
    res.set_language(ENGLISH_LANGUAGE); // US English
    res.set_icon(&cfg.icon);

    for (key, value) in &cfg.resources {
        if value.starts_with("env:") {
            let (_, env) = value.split_at(4);
            let value = env::var(env).unwrap();
            res.set(key, &value);
            println!("cargo:rerun-if-env-changed={}", env);
        } else {
            res.set(key, value);
        }
    }

    res.compile().unwrap()
}

fn generate_const<T: AsRef<str>>(name: &str, c_type: &str, value: T) -> String {
    let value = match c_type {
        "&str" => format!(r#""{}""#, value.as_ref()),
        _ => value.as_ref().into(),
    };

    format!("const {name}: {c_type} = {value};\n")
}

/* Passes values */
fn build_runtime(config: &RuntimeConfig) {
    let mut output = String::new();
    output += &generate_const("SETUP_URL", "&str", &config.setup_url);
    output += &generate_const("BASE_URL", "&str", &config.base_url);
    output += &generate_const("THREAD_COUNT", "usize", format!("{}", config.thread_count));

    fs::write(out_dir().join("./codegen.rs"), output).unwrap();
}

fn load_config() -> BuildConfig {
    println!("cargo:rerun-if-changed=build.yaml");
    let bytes = fs::read("./build.yaml").unwrap();
    serde_yaml::from_slice(&bytes).unwrap()
}

fn main() {
    let config = load_config();

    build_ascii(&config);
    build_runtime(&config.runtime);

    if let Some(_) = env::var_os("CARGO_CFG_WINDOWS") {
        build_windows(&config.windows)
    }
}
