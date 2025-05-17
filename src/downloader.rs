use std::{fs, io::Read, path::PathBuf};

use librespot_audio::{AudioDecrypt, AudioFile};
use librespot_core::{FileId, session::Session};
use librespot_metadata::{
    Metadata, Track,
    audio::{AudioFileFormat, AudioFiles},
};

#[derive(Debug)]
pub enum DlErrors {
    FetchTrack,
    TrackExists,
    NoAudioFiles,
    NoDecryptKey,
    NoEncryptedAudio,
    BufferWrite,
    Decrypting,
}

pub trait DownloadTrack {
    async fn download(&self, session: &Session, path: &PathBuf) -> Result<PathBuf, DlErrors>;
}
impl DownloadTrack for Track {
    async fn download(&self, session: &Session, path: &PathBuf) -> Result<PathBuf, DlErrors> {
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

        let key = session.audio_key().request(self.id, file).await.ok();
        let mut encrypted_data = AudioFile::open(&session, file, 320)
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

        return Ok(PathBuf::new());
    }
}
