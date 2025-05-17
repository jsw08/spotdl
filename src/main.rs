mod authentication;
mod cli;
mod downloader;
mod errors;

use authentication::authorize;
use clap::Parser;
use cli::Args;
use directories::ProjectDirs;
use downloader::{DlErrors, DownloadTrack};
use errors::Errors;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use librespot_core::{
    cache::Cache,
    spotify_id::{SpotifyId, SpotifyItemType},
};
use librespot_metadata::{Metadata, Playlist, Track};
use std::{env, fmt::Write, fs, time};

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

    let mut tracks_ids: Vec<SpotifyId> = Vec::new();
    if let Some(uri) = &args.source.uri {
        let uri = SpotifyId::from_uri(uri).map_err(|_| Errors::InvalidArguments)?;
        match uri.item_type {
            SpotifyItemType::Track => {
                let track = Track::get(&session, &uri)
                    .await
                    .map_err(|_| Errors::InvalidPlaylist)?;
                tracks_ids.push(track.id);
            }
            SpotifyItemType::Playlist => {
                let plist = Playlist::get(&session, &uri)
                    .await
                    .map_err(|_| Errors::InvalidPlaylist)?;

                tracks_ids.extend(plist.tracks().cloned());
            }
            _ => return Err(Errors::InvalidPlaylist),
        };
    }
    if tracks_ids.is_empty() {
        return Err(Errors::InvalidPlaylist);
    };

    let sty = ProgressStyle::with_template(
        "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
    )
    .unwrap()
    .progress_chars("##-");

    let pb = ProgressBar::new(tracks_ids.len() as u64);
    pb.set_style(sty.clone());
    println!("{LIGHT_GRAY_BOLD}[1/2]{RESET} 🔍 Fetching song metadata...");

    let mut tracks: Vec<Track> = Vec::new();
    for id in &tracks_ids {
        pb.inc(1);
        match Track::get(&session, &id).await {
            Ok(v) => {
                pb.set_message(format!("{}", v.name));
                tracks.push(v);
            }
            Err(e) => {
                pb.set_message(format!("ERROR: {}", id.id));
                continue;
            }
        };
    }
    pb.finish_and_clear();
    println!("{LIGHT_GRAY_BOLD}[2/2]{RESET} 🔐 Downloading and decrypting songs...");
    let pb = ProgressBar::new(tracks_ids.len() as u64);
    pb.set_style(sty.clone());

    let mut missing_tracks: Vec<Track> = Vec::new();
    let mut last_track = time::Instant::now();
    let delay = time::Duration::from_millis(args.timeout);

    for track in tracks {
        pb.set_message(format!("{}", track.name));

        if let Err(e) = track.download(&session, &path).await {
            if matches!(e, DlErrors::TrackExists) {
                continue;
            };

            pb.set_message(format!("ERROR: {:?} - {}", e, track.name));
            missing_tracks.push(track);
        };
        if pb.position() == 4 {
            eprintln!("ERROR: Downloading song.");
        }

        pb.inc(1);
        pb.set_message("sleeping 💤");
        tokio::time::sleep(delay - last_track.elapsed()).await;
        last_track = time::Instant::now();
    }

    println!("{:?}", missing_tracks);
    Ok(())
}
