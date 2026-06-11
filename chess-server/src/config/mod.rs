use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub host: String,
    pub database_url: String,
    pub jwt_secret: String,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| {
            if env::var("TEST_MODE").is_ok() {
                eprintln!("WARNING: Using default JWT secret in test mode");
                "test-secret-key-for-testing-only".to_string()
            } else {
                panic!("FATAL: JWT_SECRET not set. Set it in .env or environment.");
            }
        });
        Ok(Self {
            port: env::var("PORT").unwrap_or("3000".into()).parse()?,
            host: env::var("HOST").unwrap_or("0.0.0.0".into()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or("postgres://postgres:postgres@localhost:5432/chess".into()),
            jwt_secret,
        })
    }
}
