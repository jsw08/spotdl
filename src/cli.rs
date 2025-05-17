use clap::Parser;
use librespot_core::{Session, SpotifyId, spotify_id::SpotifyItemType};
use librespot_metadata::{Metadata, Playlist, Track};

use crate::errors::Errors;

/// Simple spotify ripper, for (relatively) high quality offline audio files!.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[clap(flatten)]
    pub source: CliSource,

    /// Path to folder where the songs gets downloaded. Will otherwise create a new folder in the current directory.
    #[arg(long)]
    pub path: Option<std::path::PathBuf>,

    /// Time to wait (in milliseconds) between downloading songs. It triggers every 10 songs. Prevents spotify from banning your account.
    #[clap(default_value_t = 5000)]
    pub timeout: u64,
}

#[derive(Debug, clap::Args)]
#[group(required = true, multiple = false)]
pub struct CliSource {
    /// From a spotify uri. Must be in the following format: "spotify:playlist:..." or ""spotify:track:...""
    #[arg(short, long)]
    pub uri: Option<String>,
}

impl Args {
    pub async fn parse_source(&self, session: &Session) -> Option<Vec<SpotifyId>> {
        let mut track_ids: Vec<SpotifyId> = Vec::new();
        let uri = &self.source.uri.clone()?;
        let uri = SpotifyId::from_uri(uri).ok()?;

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
