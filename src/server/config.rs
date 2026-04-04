pub struct Config {
    pub port: u16,
    pub data_dir: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: std::env::var("DIG2MEMORY_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(18200),
            data_dir: std::env::var("DIG2MEMORY_DATA_DIR")
                .unwrap_or_else(|_| "./data".to_string()),
        }
    }
}
