#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error(transparent)]
    TomlError(#[from] toml::ser::Error),
}
