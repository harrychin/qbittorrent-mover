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
use log::LevelFilter;
use log4rs;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::RollingFileAppender;
use log4rs::config::{Appender, Config as LogConfig, Root};
use log4rs::encode::pattern::PatternEncoder;
use std::num::ParseIntError;

const MAX_ARCHIVED_LOGS: u32 = 1;

fn parse_size(size: &str) -> Result<u64, ParseIntError> {
    let mut iter = size.chars().rev();
    let unit = iter.next();
    let number: String = iter.collect::<String>().chars().rev().collect();

    match unit {
        Some('M') => number.parse::<u64>().map(|n| n * 1024 * 1024),
        Some('G') => number.parse::<u64>().map(|n| n * 1024 * 1024 * 1024),
        _ => number.parse::<u64>(),
    }
}

pub fn setup_logger(log_file: &str, max_log_size: &str) -> Result<()> {
    if log::log_enabled!(log::Level::Info) {
        return Ok(());
    }

    let encoder = Box::new(PatternEncoder::new("{d(%Y-%m-%d %H:%M:%S)} {l} - {m}\n"));

    // Set up rolling file appender
    let roller = FixedWindowRoller::builder().build("{}.{}", MAX_ARCHIVED_LOGS)?;
    let max_log_size = parse_size(max_log_size)?;
    let trigger = SizeTrigger::new(max_log_size);
    let policy = CompoundPolicy::new(Box::new(trigger), Box::new(roller));

    let file_appender = RollingFileAppender::builder()
        .encoder(encoder)
        .build(log_file, Box::new(policy))?;

    let config = LogConfig::builder()
        .appender(Appender::builder().build("file_appender", Box::new(file_appender)))
        .build(
            Root::builder()
                .appender("file_appender")
                .build(LevelFilter::Info),
        )?;

    log4rs::init_config(config)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_setup_logger() -> Result<()> {
        let log_file = "test_logger.log";
        let max_log_size = "10M";
        setup_logger(log_file, max_log_size)?;

        // Check if the log file was created
        assert!(fs::metadata(log_file).is_ok());

        Ok(())
    }
}
