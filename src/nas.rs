use crate::{hash_file, Artifact, NasConfig};
use reqwest::{blocking::Client, header::CONTENT_TYPE, redirect::Policy};
use serde::Deserialize;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read},
    path::Path,
    time::Duration,
};

#[derive(Debug)]
pub enum NasError {
    Authentication,
    Service(String),
}

impl std::fmt::Display for NasError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authentication => {
                write!(f, "NAS rejected credentials; enter updated account details")
            }
            Self::Service(message) => write!(f, "NAS unavailable or request failed: {message}"),
        }
    }
}

impl std::error::Error for NasError {}

#[derive(Deserialize)]
struct Answer {
    success: bool,
    data: Option<Session>,
    error: Option<Code>,
}
#[derive(Deserialize)]
struct Session {
    sid: String,
}
#[derive(Deserialize)]
struct Code {
    code: u32,
}

pub struct Nas {
    client: Client,
    sid: String,
    host: String,
}

impl Nas {
    pub fn login(config: &NasConfig, username: &str, password: &str) -> Result<Self, NasError> {
        config.validate().map_err(NasError::Service)?;
        let client = Client::builder()
            .redirect(Policy::none()) // Never forward a SID to a redirected host.
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(service)?;
        let response = client
            .post(format!("{}/webapi/entry.cgi", config.host))
            .form(&[
                ("api", "SYNO.API.Auth"),
                ("version", "6"),
                ("method", "login"),
                ("account", username),
                ("passwd", password),
                ("session", "FileStation"),
                ("format", "sid"),
            ])
            .timeout(Duration::from_secs(30))
            .send()
            .map_err(service)?
            .error_for_status()
            .map_err(service)?;
        let answer: Answer = response.json().map_err(service)?;
        if !answer.success {
            return Err(login_failure(answer.error.map(|e| e.code)));
        }
        let sid = answer
            .data
            .ok_or_else(|| NasError::Service("No session data".into()))?
            .sid;
        if sid.is_empty() {
            return Err(NasError::Service("Empty session".into()));
        }
        Ok(Self {
            client,
            sid,
            host: config.host.clone(),
        })
    }

    /// `path` is a DSM FileStation path, not a filesystem path or a fbdownload URL.
    pub fn download(
        &self,
        path: &str,
        destination: &Path,
        maximum: u64,
        expected: Option<&Artifact>,
    ) -> Result<(), NasError> {
        if maximum == u64::MAX {
            return Err(NasError::Service("Unbounded download".into()));
        }
        crate::safe_name(path.strip_prefix('/').unwrap_or("")).map_err(NasError::Service)?;
        let response = self
            .client
            .get(format!("{}/webapi/entry.cgi", self.host))
            .query(&[
                ("api", "SYNO.FileStation.Download"),
                ("version", "2"),
                ("method", "download"),
                ("path", path),
                ("mode", "open"),
                ("_sid", &self.sid),
            ])
            .send()
            .map_err(service)?
            .error_for_status()
            .map_err(service)?;
        if response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.contains("json") || v.contains("html"))
        {
            return Err(NasError::Service(
                "NAS returned an error page, not a file".into(),
            ));
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(service)?;
        let result = (|| -> Result<(), NasError> {
            let count = io::copy(&mut response.take(maximum + 1), &mut file).map_err(service)?;
            if count > maximum {
                return Err(NasError::Service("Download exceeds declared limit".into()));
            }
            file.sync_all().map_err(service)?;
            if let Some(entry) = expected {
                if count != entry.bytes
                    || hash_file(destination).map_err(service)? != entry.sha256.to_ascii_lowercase()
                {
                    return Err(NasError::Service("Download size/hash mismatch".into()));
                }
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(destination);
        }
        result
    }

    pub fn logout(&self) -> Result<(), NasError> {
        self.client
            .post(format!("{}/webapi/entry.cgi", self.host))
            .form(&[
                ("api", "SYNO.API.Auth"),
                ("version", "6"),
                ("method", "logout"),
                ("session", "FileStation"),
                ("_sid", &self.sid),
            ])
            .timeout(Duration::from_secs(10))
            .send()
            .map_err(service)?
            .error_for_status()
            .map_err(service)?;
        Ok(())
    }
}

fn service(_error: impl std::fmt::Display) -> NasError {
    // HTTP errors can include the full _sid query URL; never expose that in logs or dialogs.
    NasError::Service("Network, TLS, response, or local file operation failed".into())
}

fn login_failure(code: Option<u32>) -> NasError {
    if code == Some(400) {
        NasError::Authentication
    } else {
        NasError::Service(format!(
            "Login rejected (DSM code {code:?}); do not assume changed password"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_invalid_account_password_prompts_again() {
        assert!(matches!(login_failure(Some(400)), NasError::Authentication));
        for code in [None, Some(401), Some(403), Some(407)] {
            assert!(matches!(login_failure(code), NasError::Service(_)));
        }
    }
}
