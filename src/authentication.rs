use crate::errors::Errors;
use librespot_core::{
    authentication::Credentials, cache::Cache, config::SessionConfig, session::Session,
};
use librespot_oauth::get_access_token;

const SPOTIFY_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
const CLIENT_REDIRECT_URI: &str = "http://127.0.0.1:8989/login";
pub async fn authorize(cache: &Cache) -> Result<Session, Errors> {
    let connect = async |session: Session, credentials: Credentials| -> Option<Session> {
        session
            .connect(credentials, false)
            .await
            .map(|_| session)
            .ok()
            .and_then(|v| if v.is_invalid() { None } else { Some(v) })
    };

    let session = if let Some(credentials) = cache.credentials() {
        let session = Session::new(SessionConfig::default(), None);
        connect(session, credentials).await
    } else {
        None
    };

    let session = match session {
        Some(v) => Some(v),
        None => {
            let token = get_access_token(SPOTIFY_CLIENT_ID, CLIENT_REDIRECT_URI, vec!["streaming"])
                .map_err(|_| Errors::Login)?;
            let credentials = Credentials::with_access_token(token.access_token);
            let session = Session::new(SessionConfig::default(), None);

            cache.save_credentials(&credentials);
            connect(session, credentials).await
        }
    }
    .ok_or(Errors::Login)?;

    Ok(session)
}
