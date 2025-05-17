mod authentication;
mod cli;
mod downloader;
mod errors;

use authentication::authorize;
use clap::Parser;
use cli::Args;
use directories::ProjectDirs;
use downloader::{DlErrors, DownloadTrack, DownloadTrackStatus, FetchTrackStatus, SpotifyIDs};
use errors::Errors;
use indicatif::{ProgressBar, ProgressStyle};
use librespot_core::cache::Cache;
use librespot_metadata::Track;
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
    println!("{LIGHT_GRAY_BOLD}[1/2]{RESET} 🔍 Fetching song metadata...");

    let tracks = track_ids
        .get_tracks(
            &session,
            Some(|status| {
                pb.inc(1);
                match status {
                    FetchTrackStatus::Error(err, id) => {
                        eprintln!("Failed to fetch metadata about id: {}", id);
                        pb.set_message(format!("ERROR ({id}): {err}"));
                    }
                    FetchTrackStatus::Update(name) => {
                        pb.set_message(name);
                    }
                }
            }),
        )
        .await
        .ok_or(Errors::InvalidPlaylist)?;

    pb.finish_and_clear();
    println!("{LIGHT_GRAY_BOLD}[2/2]{RESET} 🔐 Downloading and decrypting songs...");
    let pb = ProgressBar::new(tracks_len as u64);
    pb.set_style(sty.clone());

    let mut missing_tracks: Vec<Track> = Vec::new();
    let mut last_track = time::Instant::now();
    let delay = time::Duration::from_millis(args.timeout);

    for (index, track) in tracks.iter().enumerate() {
        pb.set_message(format!("{}", track.name));

        if let Err(e) = track
            .download(
                &session,
                &path,
                Some(|cback| match cback {
                    DownloadTrackStatus::Searching => {
                        pb.set_message(format!("🔎 Searching for files '{}'...", track.name))
                    }
                    DownloadTrackStatus::Downloading => {
                        pb.set_message(format!("🔐 Downloading and decrypting '{}'...", track.name))
                    }
                }),
            )
            .await
        {
            if matches!(e, DlErrors::TrackExists) {
                pb.inc(1);
                continue;
            };

            pb.set_message(format!("ERROR: {:?} - {}", e, track.name));
            missing_tracks.push(track.clone());
        };

        pb.inc(1);
        if index + 1 != tracks_len {
            pb.set_message("💤 Sleeping...");
            tokio::time::sleep(delay - last_track.elapsed()).await;
            last_track = time::Instant::now();
        }
    }

    pb.finish_and_clear();
    println!("Finished!");

    if !missing_tracks.is_empty() {
        println!("Failed to download a few songs:");
        for track in missing_tracks {
            println!(
                "{} by {} ({}).",
                track.name,
                track
                    .artists
                    .iter()
                    .map(|artist| artist.name.clone())
                    .collect::<Vec<String>>()
                    .join(", "),
                track.id.to_uri().unwrap_or("no id".to_string())
            );
        }
    }

    Ok(())
}
