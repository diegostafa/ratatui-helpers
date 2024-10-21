use std::fs;

use directories::ProjectDirs;
use serde::de::DeserializeOwned;

pub fn parse_toml<Partial: DeserializeOwned, Full: From<Partial>>(proj: &str, path: &str) -> Full {
    let proj = ProjectDirs::from("", "", proj).expect("Failed to find the project directory");
    let file = proj.config_dir().join(path);
    let content =
        &fs::read_to_string(file).expect(&format!("Failed to read the file at: {}", path));
    let toml = toml::from_str(&content);

    match toml {
        Ok(toml) => Full::from(toml),
        Err(e) => {
            println!("Failed to parse the file with error: {}", e);
            panic!();
        }
    }
}
