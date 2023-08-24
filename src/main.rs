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

mod config;
mod logger;
mod torrent;

use anyhow::{Error, Result};
use config::{ServerConfig, CONFIG_FILE};
use futures::future::join_all;
use log::{error, info};
use logger::setup_logger;
use std::time::Duration;
use tokio;
use tokio::sync::oneshot::channel as oneshot_channel;
use tokio::sync::oneshot::Receiver as OneshotReceiver;
use tokio::time::sleep;

use crate::torrent::TorrentClient;

#[tokio::main]
async fn main() -> Result<()> {
    info!("Starting qBittorrent Mover");

    let config = config::load_config(CONFIG_FILE).map_err(|e| {
        error!("Failed to load configuration: {}", e);
        anyhow::Error::from(e)
    })?;

    setup_logger(&config.log_file, &config.max_log_file_size)?;

    let (shutdown_sender, shutdown_receiver) = oneshot_channel();

    // Spawn a task to listen for the ctrl+c signal
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        let _ = shutdown_sender.send(());
    });

    main_loop(config, shutdown_receiver).await?;

    info!("Shutting down qBittorrent Mover");
    Ok(())
}

async fn process_single_server(server: ServerConfig) -> Result<(), Error> {
    let torrent_client = TorrentClient::new(server);
    let is_online = torrent::is_server_online(&torrent_client).await?;
    if is_online {
        let torrents = torrent::get_completed_torrents(&torrent_client).await?;
        for torrent in torrents {
            let torrent_client = torrent_client.clone();
            tokio::spawn(async move {
                if let Err(e) =
                    torrent::move_and_clean_torrent_files(&torrent_client, &torrent).await
                {
                    error!("Error moving and cleaning torrent files: {}", e);
                }
            });
        }
    }
    Ok(())
}

async fn process_all_servers(servers: &[ServerConfig]) -> Result<(), Error> {
    let tasks = servers
        .iter()
        .map(|server| process_single_server(server.clone()));
    let results: Vec<_> = join_all(tasks).await;

    let errors: Vec<Error> = results.into_iter().filter_map(|res| res.err()).collect();
    if !errors.is_empty() {
        return Err(anyhow::anyhow!("Encountered {} errors", errors.len()));
    }

    Ok(())
}

async fn main_loop(config: config::Config, mut shutdown_signal: OneshotReceiver<()>) -> Result<()> {
    loop {
        if let Err(e) = process_all_servers(&config.servers).await {
            error!("Error processing servers: {}", e);
        }

        tokio::select! {
            Ok(_) = &mut shutdown_signal => {
                info!("Received shutdown signal. Exiting...");
                break;
            }
            _ = sleep(Duration::from_secs(config.rate_limit_delay)) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use mockito::Server;
    use tokio;

    #[tokio::test]
    async fn test_main_loop() -> Result<()> {
        // Setup
        let (shutdown_sender, shutdown_receiver) = oneshot_channel();

        // Start the mock server
        let mut server = Server::new();
        let m1 = server
            .mock("GET", "/api/v2/app/version")
            .with_status(200)
            .expect(1)
            .create();
        let m2 = server
            .mock("GET", "/api/v2/torrents/info?filter=completed")
            .with_status(200)
            .with_body("[]")
            .expect(1)
            .create();

        // Update the config to use the mock server
        let mut config = config::Config::default();
        config.servers = vec![config::ServerConfig {
            qbit_url: server.url(),
            ..Default::default()
        }];

        // Run the main_loop
        let main_loop_future = tokio::spawn(main_loop(config, shutdown_receiver));

        // Wait for a while and then send the shutdown signal
        sleep(Duration::from_secs(1)).await;
        let _ = shutdown_sender.send(());

        // Wait for the main_loop to finish
        let _ = main_loop_future.await?;

        // Verify the mock expectations
        m1.assert();
        m2.assert();

        Ok(())
    }
}
