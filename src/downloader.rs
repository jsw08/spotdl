use std::{fs, io::Read, path::PathBuf};

use librespot_audio::{AudioDecrypt, AudioFile};
use librespot_core::{FileId, SpotifyId, session::Session};
use librespot_metadata::{
    audio::{item::AudioItemResult, AudioFileFormat, AudioFiles, AudioItem}, Metadata, Track
};

const FORMATS: [(AudioFileFormat, u16); 3] = [
    ( AudioFileFormat::OGG_VORBIS_320, 320 ),
    ( AudioFileFormat::OGG_VORBIS_160, 160 ),
    ( AudioFileFormat::OGG_VORBIS_96, 96 ),
];

#[derive(Debug)]
pub enum DlErrors {
    TrackNotFound,
    TrackExists,
    TrackUnavailable,
    NoAudioFiles,
    NoEncryptedAudio,
    BufferWrite,
    Decrypting,
    NoKey
}

pub enum FetchTrackStatus {
    Update(String),
    Error(librespot_core::Error, SpotifyId),
}
pub enum DownloadTrackStatus {
    Searching,
    Downloading(String),
}

pub trait GetTrackOrAlternative {
    async fn get_file_or_alternative(session: &Session, id: SpotifyId) -> Result<AudioItem, DlErrors>;
}
impl GetTrackOrAlternative for AudioFile {
    async fn get_file_or_alternative(session: &Session, id: SpotifyId) -> Result<AudioItem, DlErrors> {
        let item = AudioItem::get_file(&session, id)
            .await
            .map_err(|_| DlErrors::TrackNotFound)?;

        if item.availability.is_ok() && !item.files.is_empty() {
            return Ok(item)
        }; 

        if let Some(alternatives) = item.alternatives {
            let mut alternative_files: Option<AudioItem> = None;

            for alt_id in alternatives.iter() {
                let file = match AudioItem::get_file(session, *alt_id).await {
                    Ok(v) => v,
                    _ => continue
                };
                if let Err(_) = file.availability {continue};


                let mut has_ogg = false;
                for (format, _) in FORMATS {
                    if file.files.contains_key(&format) {has_ogg = true; break}
                }
                if !has_ogg {continue}

                alternative_files = Some(file);
                break
            }

            return alternative_files.ok_or(DlErrors::TrackUnavailable)
        } 

        Err(DlErrors::TrackUnavailable)
    }
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
impl DownloadTrack for SpotifyId {
    async fn download<F>(
        &self,
        session: &Session,
        path: &PathBuf,
        callback: Option<F>,
    ) -> Result<PathBuf, DlErrors>
    where
        F: Fn(DownloadTrackStatus) -> () {
            let callback = |status: DownloadTrackStatus| callback.map(|v| v(status));

            let item = AudioFile::get_file_or_alternative(&session, *self).await?;
            let mut file: Option<(&FileId, u16)> = None;
            for (format, bitrate) in FORMATS {
                println!("{:?}, {}", format, bitrate);
                let current_file_id = match item.files.get(&format) {
                    Some(v) => v,
                    None => continue
                };

                file = Some((current_file_id, bitrate));
                break
            }
            let (file_id, file_bitrate) = file.ok_or(DlErrors::NoAudioFiles)?;

            let path = path.join(format!("{} - {}.ogg", item.uri, item.name));
            if path.exists() {
                return Err(DlErrors::TrackExists);
            }

            callback(DownloadTrackStatus::Downloading(item.name.clone()));
            let key = match session.audio_key().request(*self, *file_id).await {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("Error getting key for '{}': '{}'. Trying to download without decrypting.", item.name, e);
                    None
                }

            };
            let mut encrypted_data = AudioFile::open(session, *file_id, file_bitrate as usize)
                .await
                .map_err(|_| DlErrors::NoEncryptedAudio)?;
            let mut buffer = Vec::new();
            AudioDecrypt::new(key, &mut encrypted_data)
                .read_to_end(&mut buffer)
                .map_err(|_| DlErrors::Decrypting)?;
            fs::write(&path, &buffer[0x7a..]).map_err( |_| DlErrors::BufferWrite)?;

            Ok(path)
        }
}