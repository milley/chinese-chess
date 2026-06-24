use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub host: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub cors_origins: Vec<String>,
    /// Optional trusted proxy header for client IP extraction (e.g., "x-real-ip" set by nginx).
    /// When set, only this header is trusted; X-Forwarded-For is ignored.
    /// When unset, uses the direct socket address via ConnectInfo.
    pub trusted_proxy_header: Option<String>,
    /// Maximum number of database connections in the pool.
    /// Defaults to 10. Configure via DATABASE_POOL_SIZE env var.
    pub database_pool_size: u32,
    #[allow(dead_code)] // Used in tests; may be read by future config logic
    pub test_mode: bool,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let test_mode = env::var("TEST_MODE").is_ok();
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
            if test_mode {
                eprintln!("WARNING: Using default JWT secret in test mode");
                "test-secret-key-for-testing-only".to_string()
            } else {
                panic!("FATAL: JWT_SECRET not set. Set it in .env or environment.");
            }
        });
        // CORS origins: comma-separated list, or "*" for any (test mode default)
        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| {
                if test_mode { "*".to_string() } else { String::new() }
            })
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(Self {
            port: env::var("PORT").unwrap_or("3000".into()).parse()?,
            host: env::var("HOST").unwrap_or("0.0.0.0".into()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or("postgres://postgres:postgres@localhost:5432/chess".into()),
            jwt_secret,
            cors_origins,
            trusted_proxy_header: env::var("TRUSTED_PROXY_HEADER").ok(),
            database_pool_size: env::var("DATABASE_POOL_SIZE")
                .unwrap_or("10".into())
                .parse()?,
            test_mode,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;

    /// Global mutex to serialize config tests (they modify env vars).
    static CONFIG_TEST_MUTEX: Mutex<()> = Mutex::new(());

    /// RAII guard that restores an env var to its original value on drop.
    struct EnvGuard {
        key: String,
        original: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &str, value: &str) -> Self {
            let original = env::var(key).ok();
            // SAFETY: test-only; we restore the original value on drop.
            unsafe { env::set_var(key, value); }
            Self { key: key.to_string(), original }
        }

        fn remove(key: &str) -> Self {
            let original = env::var(key).ok();
            // SAFETY: test-only; we restore the original value on drop.
            unsafe { env::remove_var(key); }
            Self { key: key.to_string(), original }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: test-only; restoring the original value we saved.
            unsafe {
                match &self.original {
                    Some(v) => env::set_var(&self.key, v),
                    None => env::remove_var(&self.key),
                }
            }
        }
    }

    #[test]
    fn test_app_config_test_mode_defaults() {
        let _lock = CONFIG_TEST_MUTEX.lock().unwrap();
        let _g1 = EnvGuard::set("TEST_MODE", "1");
        let _g2 = EnvGuard::remove("JWT_SECRET");
        let _g3 = EnvGuard::remove("CORS_ORIGINS");

        let config = AppConfig::from_env().expect("from_env should succeed");
        assert!(config.test_mode);
        assert_eq!(config.jwt_secret, "test-secret-key-for-testing-only");
        assert!(config.cors_origins.contains(&"*".to_string()));
    }

    #[test]
    fn test_app_config_cors_origins_parsing() {
        let _lock = CONFIG_TEST_MUTEX.lock().unwrap();
        let _g1 = EnvGuard::set("CORS_ORIGINS", "http://a.com,http://b.com");
        let _g2 = EnvGuard::set("JWT_SECRET", "test-secret");
        let _g3 = EnvGuard::set("TEST_MODE", "1");

        let config = AppConfig::from_env().expect("from_env should succeed");
        assert_eq!(config.cors_origins, vec!["http://a.com", "http://b.com"]);
    }
}
