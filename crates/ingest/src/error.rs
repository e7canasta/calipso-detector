//! Errors exposed at the ingestion boundary.

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IngestError {
    #[error("RTSP authorization failed: {0}")]
    Unauthorized(String),
    #[error("RTSP server unreachable: {0}")]
    Unreachable(String),
    #[error("RTSP protocol: {0}")]
    Protocol(String),
    #[error("video decode: {0}")]
    Decode(String),
}

impl IngestError {
    pub(crate) fn rtsp(error: retina::Error, connecting: bool) -> Self {
        match error.status_code() {
            Some(401 | 403) => Self::Unauthorized(error.to_string()),
            Some(_) => Self::Protocol(error.to_string()),
            None if connecting => Self::Unreachable(error.to_string()),
            None => Self::Protocol(error.to_string()),
        }
    }
}

impl From<ffmpeg::Error> for IngestError {
    fn from(error: ffmpeg::Error) -> Self {
        Self::Decode(error.to_string())
    }
}
