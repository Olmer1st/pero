//! What the page is allowed to ask the app to do. The page (or anything injected into it) can only
//! name a path on the Pero server and a method; it cannot choose another host, scheme or header.

/// `host` or `host:port` (letters, digits, dots and dashes; default port 8443), or an error.
pub fn server_address(input: &str) -> Result<String, String> {
    let text = input.trim();
    let (host, port) = match text.rsplit_once(':') {
        Some((h, p)) => (h, p),
        None => (text, "8443"),
    };
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && !host.starts_with(['.', '-'])
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    let port_ok =
        port.parse::<u16>().is_ok_and(|p| p > 0) && port.bytes().all(|b| b.is_ascii_digit());
    if host_ok && port_ok {
        Ok(format!("{host}:{port}"))
    } else {
        Err("The address should look like writer.local:8443 (a name or number, and a port).".into())
    }
}

/// Only the server's own API and health check; no query tricks, no other paths.
pub fn api_path(path: &str) -> Result<(), String> {
    let ok = (path == "/health" || path.starts_with("/api/v1/"))
        && path.len() <= 2048
        && !path.contains("..")
        && !path.contains('#')
        && path.bytes().all(|b| b.is_ascii_graphic());
    if ok {
        Ok(())
    } else {
        Err("That path is not allowed.".into())
    }
}

pub fn method(name: &str) -> Result<reqwest::Method, String> {
    match name {
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" => {
            name.parse().map_err(|_| "bad method".into())
        }
        _ => Err("That method is not allowed.".into()),
    }
}

/// `pero_` and 64 lowercase hex characters, as the server issues them.
pub fn token_shape(token: &str) -> bool {
    token.strip_prefix("pero_").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_checked_and_get_the_default_port() {
        assert_eq!(server_address("writer.local").unwrap(), "writer.local:8443");
        assert_eq!(
            server_address("  writer.local:8443 ").unwrap(),
            "writer.local:8443"
        );
        assert_eq!(
            server_address("10.0.0.106:9000").unwrap(),
            "10.0.0.106:9000"
        );
        for bad in [
            "",
            ":8443",
            "evil.com/path",
            "https://writer.local",
            "a b",
            "writer.local:0",
            "writer.local:99999",
            "writer.local:80a",
            "-x.local",
            ".local",
            "user@host",
            "[::1]:8443",
        ] {
            assert!(server_address(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn only_the_api_and_health_paths_pass() {
        for ok in [
            "/health",
            "/api/v1/me",
            "/api/v1/books/bk_1/facts?status=ai_hypothesis&stale=true",
        ] {
            assert!(api_path(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "/",
            "/index.html",
            "/api/v2/me",
            "/api/v1/../x",
            "/api/v1/a b",
            "http://evil/api/v1/me",
            "//evil/api/v1/me",
            "/api/v1/x#y",
            "/healthz",
            "/api/v1/é",
        ] {
            assert!(api_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn methods_and_tokens() {
        for m in ["GET", "POST", "PUT", "PATCH", "DELETE"] {
            assert!(method(m).is_ok());
        }
        for m in ["CONNECT", "TRACE", "get", "OPTIONS", ""] {
            assert!(method(m).is_err(), "{m}");
        }
        assert!(token_shape(&format!("pero_{}", "a1".repeat(32))));
        assert!(!token_shape("pero_short"));
        assert!(!token_shape(&format!("pero_{}", "A1".repeat(32))));
        assert!(!token_shape(&format!("tok_{}", "a1".repeat(32))));
    }
}
