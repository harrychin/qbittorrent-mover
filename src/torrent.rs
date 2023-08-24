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

use super::config::ServerConfig;
use anyhow::Result;
use reqwest::{Client, Method, Response};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Torrent {
    pub save_path: String,
    pub name: String,
    pub category: String,
    pub hash: String,
}

#[derive(Clone)]
pub struct TorrentClient {
    client: Client,
    server: ServerConfig,
}

impl TorrentClient {
    pub fn new(server: ServerConfig) -> Self {
        Self {
            client: Client::new(),
            server,
        }
    }

    async fn make_request(&self, url: &str, method: Method) -> Result<Response> {
        let request = self
            .client
            .request(method, url)
            .basic_auth(&self.server.username, Some(&self.server.password))
            .build()?;
        let response = self.client.execute(request).await?;
        Ok(response)
    }
}

pub async fn is_server_online(client: &TorrentClient) -> Result<bool> {
    let url = format!("{}/api/v2/app/version", client.server.qbit_url);
    let response = client.make_request(&url, Method::GET).await?;
    Ok(response.status().is_success())
}

pub async fn get_completed_torrents(client: &TorrentClient) -> Result<Vec<Torrent>> {
    let url = format!(
        "{}/api/v2/torrents/info?filter=completed",
        client.server.qbit_url
    );
    let response = client.make_request(&url, Method::GET).await?;
    let torrents = response.json::<Vec<Torrent>>().await?;
    Ok(torrents)
}

pub async fn remove_torrent(client: &TorrentClient, hash: &str) -> Result<()> {
    let url = format!(
        "{}/api/v2/torrents/delete?hashes={}",
        client.server.qbit_url, hash
    );
    client.make_request(&url, Method::DELETE).await?;
    Ok(())
}

pub async fn move_and_clean_torrent_files(client: &TorrentClient, torrent: &Torrent) -> Result<()> {
    if let Some(dest_path) = client.server.categories.get(&torrent.category) {
        let save_path = PathBuf::from(&torrent.save_path);
        let relative_path = match &client.server.path_prefix {
            Some(prefix) => save_path.strip_prefix(prefix)?,
            None => &save_path,
        };
        let root_path = PathBuf::from(client.server.root_path.as_deref().unwrap_or(""));
        let src = root_path.join(relative_path).join(&torrent.name);
        let dest = PathBuf::from(dest_path).join(&torrent.name);

        if !src.exists() {
            return Err(anyhow::anyhow!("Source path does not exist: {:?}", src));
        }

        if src.is_file() {
            fs::copy(&src, &dest)?;
            fs::remove_file(src)?;
        } else if src.is_dir() {
            fs_extra::dir::copy(&src, &dest, &fs_extra::dir::CopyOptions::new())?;
            fs::remove_dir_all(src)?;
        } else {
            return Err(anyhow::anyhow!(
                "Source path is not a file or directory: {:?}",
                src
            ));
        }

        remove_torrent(client, &torrent.hash).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{self, Server};

    #[tokio::test]
    async fn test_new_torrent_client() {
        let server = Server::new();
        let server_config = ServerConfig {
            qbit_url: server.url(),
            ..Default::default()
        };
        let torrent_client = TorrentClient::new(server_config.clone());
        assert_eq!(torrent_client.server, server_config);
    }

    #[tokio::test]
    async fn test_make_request() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v2/app/version")
            .with_status(200)
            .create();

        let server_config = ServerConfig {
            qbit_url: server.url(),
            ..Default::default()
        };
        let torrent_client = TorrentClient::new(server_config);
        let url = format!("{}/api/v2/app/version", torrent_client.server.qbit_url);
        let response = torrent_client.make_request(&url, Method::GET).await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_is_server_online() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v2/app/version")
            .with_status(200)
            .create();

        let server_config = ServerConfig {
            qbit_url: server.url(),
            ..Default::default()
        };
        let torrent_client = TorrentClient::new(server_config);
        let is_online = is_server_online(&torrent_client).await;
        assert!(is_online.is_ok());
    }

    #[tokio::test]
    async fn test_get_completed_torrents() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/api/v2/torrents/info?filter=completed")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create();

        let server_config = ServerConfig {
            qbit_url: server.url(),
            ..Default::default()
        };
        let torrent_client = TorrentClient::new(server_config);
        let torrents = get_completed_torrents(&torrent_client).await;
        assert!(torrents.is_ok());
    }

    #[tokio::test]
    async fn test_remove_torrent() {
        let mut server = Server::new();
        let _m = server
            .mock("DELETE", "/api/v2/torrents/delete?hashes=test_hash")
            .with_status(200)
            .create();

        let server_config = ServerConfig {
            qbit_url: server.url(),
            ..Default::default()
        };
        let torrent_client = TorrentClient::new(server_config);
        let hash = "test_hash";
        let result = remove_torrent(&torrent_client, hash).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_move_and_clean_torrent_files() -> Result<()> {
        let server = Server::new();
        let server_config = ServerConfig {
            qbit_url: server.url(),
            ..Default::default()
        };
        let torrent_client = TorrentClient::new(server_config);

        // Setup
        let tmp_file = tempfile::NamedTempFile::new()?;
        let tmp_dir = tmp_file.path().parent().unwrap();
        println!("Temporary Directory: {:?}", tmp_dir);
        let src_dir = tmp_dir.join("src");
        let dest_dir = tmp_dir.join("dest");
        println!("Source Directory: {:?}", src_dir);
        println!("Destination Directory: {:?}", dest_dir);
        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&dest_dir)?;

        let torrent = Torrent {
            save_path: src_dir.to_str().unwrap().to_string(),
            name: String::from("test_torrent"),
            category: String::from("test_category"),
            hash: String::from("test_hash"),
        };

        // Create a file in the src directory
        let src_file = src_dir.join(&torrent.name);
        fs::File::create(&src_file)?;

        // Update the server config to include the dest directory
        let mut server_config = torrent_client.server.clone();
        server_config.categories.insert(
            torrent.category.clone(),
            dest_dir.to_str().unwrap().to_string(),
        );
        let torrent_client = TorrentClient::new(server_config);

        // Move and clean the torrent files
        move_and_clean_torrent_files(&torrent_client, &torrent).await?;

        // Check if the file was moved
        assert!(!src_file.exists());
        assert!(dest_dir.join(&torrent.name).exists());

        Ok(())
    }
}
