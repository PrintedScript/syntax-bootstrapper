use std::{ result, error::Error };

use colored::*;

type Result<T> = result::Result<T, Box<dyn Error>>;

pub fn get_time() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn get_progress_style<T: AsRef<str>>(target: T) -> Result<ProgressStyle> {
    let progress_style = indicatif::ProgressStyle
        ::default_bar()
        .template(
            format!(
                "{}\n{}",
                format!(
                    "[{}] [{}] {}",
                    get_time().bold().blue(),
                    "INFO".bold().green(),
                    format!("Downloading {}", target.as_ref())
                ),
                " ".repeat(16) +
                    "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})"
            ).as_str()
        )?
        .progress_chars("#>-");

    Ok(progress_style)
}

pub fn create_progress() {}

#[macro_export]
macro_rules! info {
    ($($t:tt)*) => {
        {
           use ::colored::*;
           use crate::log::get_time;
           println!( "[{}] [{}] {}",get_time().bold().blue(),"INFO".bold().green(),format!($($t)*));
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($t:tt)*) => {
        {
           use ::colored::*;
           use crate::log::get_time;
           println!( "[{}] [{}] {}",get_time().bold().blue(),"ERROR".bold().red(),format!($($t)*));
        }
    };
}

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {
        {
           use ::colored::*;
           use crate::log::get_time;
           println!( "[{}] [{}] {}",get_time().bold().blue(),"DEBUG".bold().yellow(),format!($($t)*));
        }
    };
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {
        {
           
        }
    };
}
#[macro_export]
macro_rules! fatal {
    ($($t:tt)*) => {
        {
           use ::colored::*;
           use crate::log::get_time;
           panic!( "[{}] [{}] {}",get_time().bold().blue(),"FATAL".bold().red().underline(),format!($($t)*));
        }
    };
}

use indicatif::ProgressStyle;
pub(crate) use info;
pub(crate) use error;
pub(crate) use debug;
pub(crate) use fatal;
