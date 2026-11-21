/// Why a file could not be opened.
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    /// The file could not be read.
    #[error("cannot read {0}: {1}")]
    Io(String, #[source] std::io::Error),
    /// No decoder handles this file.
    #[error("unsupported media: {0}")]
    Unsupported(String),
    /// The decoder failed part way.
    #[error("decode error: {0}")]
    Decode(String),
    /// `ffmpeg` or `ffprobe` is not installed or not on the PATH.
    #[error("ffmpeg is not available: {0}")]
    NoFfmpeg(String),
}
