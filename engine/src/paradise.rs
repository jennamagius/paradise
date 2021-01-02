pub struct Paradise {
    i: ::std::sync::Arc<Internal>,
}

struct Internal {
    config: crate::config::Config,
    db_pool: ::sqlx::Pool<::sqlx::Postgres>,
}

impl Paradise {
    pub async fn new(config: crate::config::Config) -> anyhow::Result<Self> {
        let db_pool = match ::sqlx::pool::Pool::connect(&config.db).await {
            Ok(x) => {
                log::debug!("Database connection established.");
                x
            }
            Err(err) => {
                log::error!("Database connection failed: {}.", err);
                return Err(anyhow::Error::new(err));
            }
        };
        let i = Internal { config, db_pool };
        let i = ::std::sync::Arc::new(i);
        let result = Paradise { i };
        Ok(result)
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        Ok(())
    }
}
