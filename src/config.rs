/*
qBittorrent Mover - A tool to automatically move torrents to different categories based on their state.
Copyright (C) 2023 Harrison Chin

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published
by the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_yaml;
use std::collections::HashMap;
use std::fs::File;

pub const CONFIG_FILE: &str = "config.yaml";

#[derive(Debug, Deserialize, Clone, Serialize)]
pub struct Config {
    pub servers: Vec<ServerConfig>,
    pub rate_limit_delay: u64,
    pub log_file: String,
    pub max_log_file_size: String, // Size as a string, like "10MB", "1GB", etc.
}

impl Default for Config {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
            rate_limit_delay: 5,
            log_file: String::from("qbittorrent-mover.log"),
            max_log_file_size: String::from("10M"),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Serialize, PartialEq)]
pub struct ServerConfig {
    pub qbit_url: String,
    pub username: String,
    pub password: String,
    pub categories: HashMap<String, String>,
    pub root_path: Option<String>,
    pub path_prefix: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            qbit_url: String::from("http://localhost:8080"),
            username: String::from("admin"),
            password: String::from("adminadmin"),
            categories: HashMap::new(),
            root_path: None,
            path_prefix: None,
        }
    }
}

pub fn load_config(filename: &str) -> Result<Config> {
    let file = File::open(filename);
    match file {
        Ok(file) => serde_yaml::from_reader(file).map_err(|e| e.into()),
        Err(_) => {
            let default_config = Config::default();
            let file = File::create(filename)?;
            serde_yaml::to_writer(&file, &default_config)?;
            Ok(default_config)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.servers.is_empty());
        assert_eq!(config.rate_limit_delay, 5);
        assert_eq!(config.log_file, "qbittorrent-mover.log");
        assert_eq!(config.max_log_file_size, "10M");
    }

    #[test]
    fn test_default_server_config() {
        let server_config = ServerConfig::default();
        assert_eq!(server_config.qbit_url, "http://localhost:8080");
        assert_eq!(server_config.username, "admin");
        assert_eq!(server_config.password, "adminadmin");
        assert_eq!(server_config.categories, HashMap::new());
    }
    #[test]
    fn test_load_config() {
        let mut test_config = Config::default();
        test_config.servers.push(ServerConfig::default());
        let filename = "test_config.yaml";
        let file = File::create(filename).expect("Failed to create file");
        serde_yaml::to_writer(file, &test_config).expect("Failed to write to file");

        let config = load_config(filename);
        assert!(config.is_ok());
        let config = config.expect("Failed to load config");
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.rate_limit_delay, 5);
        assert_eq!(config.log_file, "qbittorrent-mover.log");
        assert_eq!(config.max_log_file_size, "10M");

        fs::remove_file(filename).expect("Failed to remove file");
    }
    #[test]
    fn test_load_config_creates_file() {
        let filename = "test_config_create.yaml";
        let _ = fs::remove_file(filename); // Ensure the file does not exist before the test

        let config = load_config(filename);
        assert!(config.is_ok(), "Failed to load config");
        assert!(fs::metadata(filename).is_ok(), "File was not created");

        fs::remove_file(filename).expect("Failed to remove file");
    }
}
