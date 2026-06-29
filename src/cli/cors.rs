use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct CorsConfig {
    /// Allowed CORS origins (comma-separated).
    /// For local development, localhost and 127.0.0.1 are always allowed.
    /// Use specific origins in production (e.g., "https://example.com,https://app.example.com").
    /// IMPORTANT: Never use "*" wildcard in production.
    #[arg(
        long = "cors-origins",
        env = "CORS_ALLOWED_ORIGINS",
        value_delimiter = ',',
        default_value = ""
    )]
    pub allowed_origins: Vec<String>,

    /// Allow credentials (cookies, authorization headers) in CORS requests.
    /// Disabled by default for security. Only enable if you need authenticated cross-origin requests.
    #[arg(
        long = "cors-allow-credentials",
        env = "CORS_ALLOW_CREDENTIALS",
        default_value = "false"
    )]
    pub allow_credentials: bool,

    /// Maximum age (in seconds) for CORS preflight cache.
    /// Default is 3600 seconds (1 hour).
    #[arg(long = "cors-max-age", env = "CORS_MAX_AGE", default_value = "3600")]
    pub max_age: u64,
}

impl CorsConfig {
    /// Returns the list of allowed origins, including developer-friendly defaults.
    /// If no origins are configured, returns localhost variants for local development.
    pub fn get_allowed_origins(&self) -> Vec<String> {
        let mut origins = self.allowed_origins.clone();

        // Remove any empty strings that might come from parsing
        origins.retain(|s| !s.is_empty());

        // If no origins specified, use developer-friendly defaults
        if origins.is_empty() {
            origins = vec![
                "http://localhost:3000".to_string(),
                "http://localhost:8080".to_string(),
                "http://127.0.0.1:3000".to_string(),
                "http://127.0.0.1:8080".to_string(),
            ];
        }

        origins
    }

    /// Validates that the configuration doesn't use wildcard in what appears to be production.
    /// Returns true if the configuration looks safe.
    pub fn is_safe(&self) -> bool {
        // Check if any origin is a wildcard
        !self.allowed_origins.iter().any(|origin| origin == "*")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_origins_when_empty() {
        let config = CorsConfig {
            allowed_origins: vec![],
            allow_credentials: false,
            max_age: 3600,
        };

        let origins = config.get_allowed_origins();
        assert_eq!(origins.len(), 4);
        assert!(origins.contains(&"http://localhost:3000".to_string()));
        assert!(origins.contains(&"http://localhost:8080".to_string()));
        assert!(origins.contains(&"http://127.0.0.1:3000".to_string()));
        assert!(origins.contains(&"http://127.0.0.1:8080".to_string()));
    }

    #[test]
    fn test_custom_origins() {
        let config = CorsConfig {
            allowed_origins: vec![
                "https://example.com".to_string(),
                "https://app.example.com".to_string(),
            ],
            allow_credentials: false,
            max_age: 3600,
        };

        let origins = config.get_allowed_origins();
        assert_eq!(origins.len(), 2);
        assert!(origins.contains(&"https://example.com".to_string()));
        assert!(origins.contains(&"https://app.example.com".to_string()));
    }

    #[test]
    fn test_wildcard_is_unsafe() {
        let config = CorsConfig {
            allowed_origins: vec!["*".to_string()],
            allow_credentials: false,
            max_age: 3600,
        };

        assert!(!config.is_safe());
    }

    #[test]
    fn test_safe_origins() {
        let config = CorsConfig {
            allowed_origins: vec!["https://example.com".to_string()],
            allow_credentials: false,
            max_age: 3600,
        };

        assert!(config.is_safe());
    }
}
