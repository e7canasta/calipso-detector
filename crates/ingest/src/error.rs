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
    /// The failure of the connecting phase (DESCRIBE): an error without a
    /// status code means the server could not be reached at all.
    pub(crate) fn connecting(error: retina::Error) -> Self {
        Self::for_status(&error).unwrap_or_else(|| Self::Unreachable(error.to_string()))
    }

    /// The failure of an established session (SETUP, PLAY or the stream
    /// itself): a transport error there is a protocol failure.
    pub(crate) fn streaming(error: retina::Error) -> Self {
        Self::for_status(&error).unwrap_or_else(|| Self::Protocol(error.to_string()))
    }

    /// How a response that carries a status code reads.
    fn for_status(error: &retina::Error) -> Option<Self> {
        match error.status_code() {
            Some(401 | 403) => Some(Self::Unauthorized(error.to_string())),
            Some(_) => Some(Self::Protocol(error.to_string())),
            None => None,
        }
    }
}

impl From<ffmpeg::Error> for IngestError {
    fn from(error: ffmpeg::Error) -> Self {
        Self::Decode(error.to_string())
    }
}
