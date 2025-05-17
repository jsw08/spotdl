use std::{fs, io::Read, path::PathBuf};

use librespot_audio::{AudioDecrypt, AudioFile};
use librespot_core::{FileId, SpotifyId, session::Session};
use librespot_metadata::{
    Metadata, Track,
    audio::{AudioFileFormat, AudioFiles},
};

#[derive(Debug)]
pub enum DlErrors {
    TrackExists,
    NoAudioFiles,
    NoEncryptedAudio,
    BufferWrite,
    Decrypting,
}

pub enum FetchTrackStatus {
    Update(String),
    Error(librespot_core::Error, SpotifyId),
}
pub enum DownloadTrackStatus {
    Searching,
    Downloading,
}

pub trait DownloadTrack {
    async fn download<F>(
        &self,
        session: &Session,
        path: &PathBuf,
        callback: Option<F>,
    ) -> Result<PathBuf, DlErrors>
    where
        F: Fn(DownloadTrackStatus) -> ();
}
impl DownloadTrack for Track {
    async fn download<F>(
        &self,
        session: &Session,
        path: &PathBuf,
        callback: Option<F>,
    ) -> Result<PathBuf, DlErrors>
    where
        F: Fn(DownloadTrackStatus),
    {
        let callback = |status: DownloadTrackStatus| {
            if let Some(cback) = &callback {
                cback(status)
            }
        };

        callback(DownloadTrackStatus::Searching);
        let mut file: Option<(u8, FileId)> = None;
        let mut update_file = |files: AudioFiles| {
            if files.is_empty() {
                return;
            };

            for i in files.iter() {
                let ranking = match i.0 {
                    AudioFileFormat::OGG_VORBIS_320 => 0,
                    AudioFileFormat::OGG_VORBIS_160 => 1,
                    AudioFileFormat::OGG_VORBIS_96 => 2,
                    _ => continue,
                };

                match file {
                    Some(i) if i.0 >= ranking => continue,
                    _ => {}
                };
                file = Some((ranking, *i.1))
            }
        };
        for i in self.alternatives.iter() {
            let _ = Track::get(session, i)
                .await
                .map(|track| update_file(track.files));
        }
        update_file(self.files.clone());
        let (_, file) = file.ok_or(DlErrors::NoAudioFiles)?;

        let path = path.join(format!("{}_{}.ogg", self.album.name, self.name));
        if path.exists() {
            return Err(DlErrors::TrackExists);
        }

        callback(DownloadTrackStatus::Downloading);
        let key = session.audio_key().request(self.id, file).await.ok();
        let mut encrypted_data = AudioFile::open(session, file, 320)
            .await
            .map_err(|_| DlErrors::NoEncryptedAudio)?;
        let mut buffer = Vec::new();
        AudioDecrypt::new(key, &mut encrypted_data)
            .read_to_end(&mut buffer)
            .map_err(|_| DlErrors::Decrypting)?;
        fs::write(&path, &buffer[0xa7..]).map_err(|e| {
            eprintln!("{:?}", e);
            DlErrors::BufferWrite
        })?;

        return Ok(path);
    }
}

pub trait SpotifyIDs {
    async fn get_tracks<F>(&self, session: &Session, callback: Option<F>) -> Option<Vec<Track>>
    where
        F: Fn(FetchTrackStatus) -> ();
}
impl SpotifyIDs for Vec<SpotifyId> {
    async fn get_tracks<F>(&self, session: &Session, callback: Option<F>) -> Option<Vec<Track>>
    where
        F: Fn(FetchTrackStatus) -> (),
    {
        let callback = |status: FetchTrackStatus| {
            if let Some(cback) = &callback {
                cback(status)
            }
        };
        let mut tracks: Vec<Track> = Vec::new();

        for id in self {
            match Track::get(&session, &id).await {
                Ok(v) => {
                    callback(FetchTrackStatus::Update(v.name.clone()));
                    tracks.push(v);
                }
                Err(e) => {
                    callback(FetchTrackStatus::Error(e, *id));
                    continue;
                }
            };
        }

        if tracks.is_empty() {
            None
        } else {
            Some(tracks)
        }
    }
}
