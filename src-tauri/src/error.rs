use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("could not locate home directory")]
    NoHome,

    #[error("tmux is required but was not found on PATH")]
    TmuxMissing,

    #[error("tmux failed: {0}")]
    Tmux(String),

    #[error("project {0} has no resolved filesystem path")]
    UnresolvedProject(String),
}

// Tauri commands need the error to cross the IPC boundary as a plain string.
impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
