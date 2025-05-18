use crate::prelude::*;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use regex::Regex;
use reqwest::{Certificate, Client, ClientBuilder};
/// LCU (League Client Update) API client implementation
///
/// The LCU API is the interface exposed by the League of Legends client for third-party
/// applications to interact with it. It was introduced with the updated League Client in 2016,
/// replacing the old Adobe AIR client. The API allows for:
/// - Monitoring game state
/// - Accepting match queues
/// - Managing runes and loadouts
/// - And other client-side operations
///
/// This module provides a Rust implementation for interacting with the LCU API.
use std::process::Command;
use std::time::Duration;
use tokio::time;

/// Client for interacting with the League of Legends Client API (LCU)
///
/// This struct provides methods to authenticate with and make requests to the League Client.
/// It handles the authentication token generation, port discovery, and HTTPS requests.
#[derive(Debug, Clone)]
pub struct LcuClient {
    /// Base64 encoded authentication token in format "riot:{auth-token}"
    auth_token: String,
    /// Port number where the League Client API is listening
    port: String,
    /// HTTP client configured to accept invalid SSL certificates (required for LCU)
    client: Client,
}

impl LcuClient {
    /// Creates a new LCU client instance
    ///
    /// This will attempt to:
    /// 1. Find the running League Client process
    /// 2. Extract authentication details from its command line
    /// 3. Set up an HTTPS client with proper configuration
    pub async fn new() -> Result<Self> {
        let (auth_token, port) = Self::get_league_auth()?;
        let cert = Certificate::from_pem(include_bytes!("../riotgames.pem"))?;
        let client = ClientBuilder::new()
            .add_root_certificate(cert)
            .build()
            .map_err(|e| Error::Other(e.to_string()))?;

        Ok(Self {
            auth_token,
            port,
            client,
        })
    }

    /// Extracts authentication details from the League Client process
    ///
    /// Uses Windows Management Instrumentation (wmic) to get the process command line
    /// and extracts the port and auth token using regex
    fn get_league_auth() -> Result<(String, String)> {
        let output = Command::new("wmic")
            .args([
                "process",
                "where",
                "name='LeagueClientUx.exe'",
                "get",
                "commandline",
            ])
            .output()
            .map_err(|e| Error::Other(format!("Failed to execute wmic command: {}", e)))?;

        let output_str = String::from_utf8_lossy(&output.stdout);

        // Check if the output contains "Invalid query" which indicates no League Client process
        if output_str.contains("Invalid query") || output_str.trim().is_empty() {
            return Err(Error::Other("League Client is not running".to_string()));
        }

        let port_re = Regex::new(r"--app-port=(\d+)").unwrap();
        let auth_re = Regex::new(r"--remoting-auth-token=([\w-]+)").unwrap();

        let port = port_re
            .captures(&output_str)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| {
                Error::Other("League Client found but port not found in command line".to_string())
            })?;

        let auth_token = auth_re
            .captures(&output_str)
            .and_then(|cap| cap.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| {
                Error::Other(
                    "League Client found but auth token not found in command line".to_string(),
                )
            })?;

        let auth = format!("riot:{}", auth_token);
        let auth_base64 = BASE64.encode(auth);

        Ok((auth_base64, port))
    }

    /// Makes a single request to the League Client API
    ///
    /// # Arguments
    /// * `method` - HTTP method ("GET", "POST", etc.)
    /// * `endpoint` - API endpoint (e.g., "/lol-gameflow/v1/gameflow-phase")
    /// * `body` - Optional JSON body for POST requests
    ///
    /// # Returns
    /// * Tuple of (status_code, response_body)
    pub async fn request(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<String>,
    ) -> Result<(u16, String)> {
        let url = format!("https://127.0.0.1:{}{}", self.port, endpoint);

        let mut request = self
            .client
            .request(method.parse().unwrap(), &url)
            .header("Authorization", format!("Basic {}", self.auth_token));

        if let Some(body_content) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(body_content);
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| Error::Other(e.to_string()))?;

        Ok((status, body))
    }

    /// Makes requests to the API until a successful response is received
    ///
    /// Useful for operations that might temporarily fail but should eventually succeed,
    /// such as checking game state during champion select
    pub async fn request_until_success(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<String>,
    ) -> Result<(u16, String)> {
        loop {
            match self.request(method, endpoint, body.clone()).await {
                Ok((status, response)) if status >= 200 && status < 300 => {
                    return Ok((status, response))
                }
                Ok(_) => {
                    if self.is_client_running()? {
                        time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                    return Err(Error::Other("League client is not running".to_string()));
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Checks if the League Client process is currently running
    pub fn is_client_running(&self) -> Result<bool> {
        let output = Command::new("tasklist")
            .output()
            .map_err(|e| Error::Other(e.to_string()))?;

        let output_str = String::from_utf8_lossy(&output.stdout);
        Ok(output_str.contains("LeagueClientUx.exe"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    /// Helper function to create a test client instance
    async fn get_test_client() -> LcuClient {
        LcuClient::new().await.unwrap()
    }

    /// Tests that we can detect if the League Client is running
    #[test]
    async fn test_client_running_detection() {
        let client = get_test_client().await;
        let result = client.is_client_running();
        assert!(result.is_ok());
    }

    /// Tests that we can parse authentication details from a running client
    #[test]
    async fn test_auth_parsing() {
        let client = get_test_client().await;
        if client.is_client_running().unwrap() {
            let auth_result = LcuClient::get_league_auth();
            assert!(auth_result.is_ok());

            let (auth_token, port) = auth_result.unwrap();
            assert!(!auth_token.is_empty());
            assert!(!port.is_empty());
            assert!(port.parse::<u16>().is_ok());
        }
    }

    /// Tests that we can create a new client instance
    #[test]
    async fn test_client_creation() {
        let client = get_test_client().await;
        if client.is_client_running().unwrap() {
            let client = LcuClient::new().await;
            assert!(client.is_ok());
        }
    }
}
