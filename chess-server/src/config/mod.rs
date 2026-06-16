use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub host: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub cors_origins: Vec<String>,
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
            test_mode,
        })
    }
}
