mod authentication;
mod cli;
mod downloader;
mod errors;

use authentication::authorize;
use clap::Parser;
use cli::Args;
use directories::ProjectDirs;
use downloader::{DlErrors, DownloadTrack, DownloadTrackStatus, FetchTrackStatus, };
use errors::Errors;
use indicatif::{ProgressBar, ProgressStyle};
use librespot_core::{cache::Cache, SpotifyId};
use librespot_metadata::{Metadata, Track};
use std::{env, fs, time};

const LIGHT_GRAY_BOLD: &str = "\x1b[37m";
const RESET: &str = "\x1b[0m";

#[tokio::main]
async fn main() -> Result<(), Errors> {
    let args = Args::parse();

    let config = ProjectDirs::from("tf", "jsw", "spotdl").ok_or(Errors::ConfigError)?;
    let config = config.config_dir();
    fs::create_dir_all(config).map_err(|_| Errors::ConfigError)?;

    let cache = Cache::new(Some(&config), None, None, None).map_err(|_| Errors::ConfigError)?;
    let session = authorize(&cache).await?;

    let path = match &args.path {
        Some(v) => {
            if v.is_file() {
                eprintln!("Please provide a path to a directory.");
                return Err(Errors::InvalidArguments);
            };

            v.clone()
        }
        None => env::current_dir().expect("Unable to get current directory"),
    };

    let track_ids = args
        .parse_source(&session)
        .await
        .ok_or(Errors::InvalidPlaylist)?;
    let tracks_len = track_ids.len();

    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
    )
    .unwrap()
    .progress_chars("##-");

    let pb = ProgressBar::new(tracks_len as u64);
    pb.set_style(sty.clone());

    let mut missing_tracks: Vec<(SpotifyId, String)> = Vec::new();
    let mut last_track = time::Instant::now();
    let delay = time::Duration::from_millis(args.timeout);

    for (index, track) in track_ids.iter().enumerate() {
        let name: String = Track::get(&session, track).await.map(|v| v.name).unwrap_or("track not found".to_string());

        if let Err(e) = track
            .download(
                &session,
                &path,
                Some(|cback| match cback {
                    DownloadTrackStatus::Searching => {
                        pb.set_message(format!("🔎 Searching for song {name}..."))
                    }
                    DownloadTrackStatus::Downloading(given_name) => {
                        pb.set_message(format!("🔐 Downloading and decrypting '{}'...", given_name));
                        // name = Some(given_name);
                    }
                }),
            )
            .await
        {
            if matches!(e, DlErrors::TrackExists) {
                pb.inc(1);
                continue;
            };

            println!("ERROR: {:?} - {} ({})", e, name, track);
            missing_tracks.push((track.clone(), name));
        };

        pb.inc(1);
        if index + 1 != tracks_len {
            pb.set_message("💤 Sleeping...");
            let timeout = if last_track.elapsed() >= delay {
                time::Duration::from_secs(0)
            } else {
                delay - last_track.elapsed()
            };
            tokio::time::sleep(timeout).await;
            last_track = time::Instant::now();
        }
    }

    pb.finish_and_clear();
    println!("Finished!");

    if !missing_tracks.is_empty() {
        println!("Failed to download a few songs:");
        for (track, name) in missing_tracks {
            println!(
                "{} ({}).",
                name,
                track.to_uri().unwrap_or("no id".to_string())
            );
        }
    }

    Ok(())
}
