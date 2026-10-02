use std::{env, net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub resources_dir: PathBuf,
    pub environment: String,
    pub git_sha: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let in_container = env::var_os("SERVICE_MATRIX_IN_CONTAINER").is_some();
        let default_data = if in_container { "/app/data" } else { "./data" };
        let default_resources = if in_container {
            "/app/resources"
        } else {
            "./resources"
        };
        let port = env::var("SERVICE_MATRIX_PORT")
            .unwrap_or_else(|_| "8080".to_owned())
            .parse::<u16>()
            .map_err(|error| format!("SERVICE_MATRIX_PORT must be a valid port: {error}"))?;
        let git_sha = sanitize_git_sha(env::var("SERVICE_MATRIX_GIT_SHA").unwrap_or_else(|_| {
            option_env!("SERVICE_MATRIX_GIT_SHA")
                .unwrap_or("unknown")
                .into()
        }));

        Ok(Self {
            port,
            data_dir: env::var("SERVICE_MATRIX_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(default_data)),
            resources_dir: env::var("SERVICE_MATRIX_RESOURCES_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(default_resources)),
            environment: env::var("SERVICE_MATRIX_ENVIRONMENT")
                .unwrap_or_else(|_| "Development".to_owned()),
            git_sha,
        })
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], self.port))
    }
}

fn sanitize_git_sha(value: String) -> String {
    let value = value.trim();
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        value.to_owned()
    } else {
        "unknown".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_git_sha;

    #[test]
    fn accepts_a_40_character_hex_sha() {
        let sha = "0123456789abcdef0123456789abcdef01234567".to_owned();
        assert_eq!(sanitize_git_sha(sha.clone()), sha);
    }

    #[test]
    fn rejects_invalid_sha_values() {
        assert_eq!(sanitize_git_sha("short".to_owned()), "unknown");
        assert_eq!(
            sanitize_git_sha("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".to_owned()),
            "unknown"
        );
    }
}
