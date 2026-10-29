//! Server settings, read from a TOML file outside the repository (default `/etc/pero/server.toml`).

use serde::Deserialize;
use std::{fs, net::SocketAddr, path::PathBuf};

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Address the HTTPS server listens on, e.g. `0.0.0.0:8443`.
    pub bind: SocketAddr,
    /// PEM certificate chain (server certificate first).
    pub tls_cert: PathBuf,
    /// PEM private key of the server certificate.
    pub tls_key: PathBuf,
    /// `host:port` of the local LLM service (llama.cpp server).
    pub llm: String,
    /// `host:port` of the local LanguageTool service.
    pub languagetool: String,
    /// Folder for the SQLite database and the book repositories. Without it the manuscript API is off.
    pub data_dir: Option<PathBuf>,
}

impl Config {
    pub fn load(path: &str) -> Result<Config, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        text.parse()
    }
}

impl std::str::FromStr for Config {
    type Err = String;
    fn from_str(text: &str) -> Result<Config, String> {
        toml::from_str(text).map_err(|e| format!("invalid config: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
bind = "0.0.0.0:8443"
tls_cert = "/etc/pero/tls/server.crt"
tls_key = "/etc/pero/tls/server.key"
llm = "127.0.0.1:8080"
languagetool = "127.0.0.1:8010"
"#;

    #[test]
    fn parses_a_complete_file() {
        let c: Config = GOOD.parse().unwrap();
        assert_eq!(c.bind.port(), 8443);
        assert_eq!(c.llm, "127.0.0.1:8080");
    }

    #[test]
    fn data_dir_is_optional() {
        let c: Config = GOOD.parse().unwrap();
        assert_eq!(c.data_dir, None);
        let c: Config = format!("{GOOD}data_dir = \"/var/lib/pero/data\"\n")
            .parse()
            .unwrap();
        assert_eq!(c.data_dir, Some(PathBuf::from("/var/lib/pero/data")));
    }

    #[test]
    fn rejects_a_missing_field() {
        let err = GOOD
            .replace("llm = \"127.0.0.1:8080\"\n", "")
            .parse::<Config>()
            .unwrap_err();
        assert!(err.contains("llm"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_field() {
        let err = format!("{GOOD}extra = 1\n").parse::<Config>().unwrap_err();
        assert!(err.contains("extra"), "{err}");
    }
}
