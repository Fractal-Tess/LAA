#![allow(unused)]

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Other(String),

    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error("League Client is not running")]
    ClientNotRunning,

    #[error("Failed to authenticate with League Client: {0}")]
    AuthenticationError(String),

    #[error("Request failed: {0}")]
    RequestError(String),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
}
    
