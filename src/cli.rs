use clap::Parser;
use librespot_core::{Session, SpotifyId, spotify_id::SpotifyItemType};
use librespot_metadata::{Metadata, Playlist, Track};
use regex::Regex;

use crate::errors::Errors;

/// A simple Spotify ripper designed for downloading spotify-quality offline audio files.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// A Spotify share URL or URI. This supports both playlists and individual tracks.
    /// For example: "spotify:playlist:code" or "https://open.spotify.com/track/code".
    #[clap()]
    pub source: String,

    /// The directory path where the downloaded songs will be saved. If not specified,
    /// the files will be downloaded to the current directory.
    #[arg(short, long)]
    pub path: Option<std::path::PathBuf>,

    /// The maximum time allowed for downloading a song, specified in milliseconds.
    /// For instance, if a song takes 2 seconds to download, the program will wait an
    /// additional 3 seconds by default. This helps prevent Spotify from rate limiting
    /// or banning your account.
    #[arg(short, long, default_value_t = 5000)]
    pub timeout: u64,
}

impl Args {
    pub fn get_source(&self) -> Result<SpotifyId, Errors> {
        let mut source = self.source.clone();

        if source.contains("open.spotify.com") {
            let cap =
                Regex::new(r"(?:https?:\/\/)?(?:www.)?open.spotify.com\/(\w+)\/(\w+)(?:\?.+)?")
                    .ok()
                    .and_then(|re| re.captures(&self.source))
                    .ok_or(Errors::InvalidArguments)?;
            let category = cap.get(1).map_or("", |m| m.as_str());
            let id = cap.get(2).map_or("", |m| m.as_str());

            source = format!("spotify:{category}:{id}");
        }
        let uri = SpotifyId::from_uri(&source).map_err(|_| Errors::InvalidArguments)?;

        match uri.item_type {
            SpotifyItemType::Playlist | SpotifyItemType::Track => (),
            _ => return Err(Errors::InvalidArguments),
        }

        return Ok(uri);
    }
    pub async fn parse_source(&self, session: &Session) -> Option<Vec<SpotifyId>> {
        // TODO: Move to Result
        let mut track_ids: Vec<SpotifyId> = Vec::new();
        let uri = self.get_source().ok()?;

        match uri.item_type {
            SpotifyItemType::Track => {
                let track = Track::get(session, &uri).await.ok()?;
                track_ids.push(track.id);
            }
            SpotifyItemType::Playlist => {
                let plist = Playlist::get(&session, &uri).await.ok()?;
                track_ids.extend(plist.tracks().cloned());
            }
            _ => return None,
        };

        if track_ids.is_empty() {
            None
        } else {
            Some(track_ids)
        }
    }
}
