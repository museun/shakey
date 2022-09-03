use std::{path::PathBuf, sync::Arc, time::Duration};

use tokio::sync::RwLock;

use crate::{get_env_var, watch_file};

use super::{FileTypes, Interest, WatchFile};

#[async_trait::async_trait]
pub trait Watch<const FORMAT: u8>
where
    Self: Sized + Interest,
{
    async fn watch() -> anyhow::Result<WatchFile<Self, FORMAT>>;
}

#[async_trait::async_trait]
impl<T, const FORMAT: u8> Watch<FORMAT> for T
where
    T: Interest + for<'de> serde::Deserialize<'de>,
    T: Send + Sync + 'static,
{
    async fn watch() -> anyhow::Result<WatchFile<Self, FORMAT>> {
        const SLEEP: Duration = Duration::from_secs(1);
        const MODIFICATION: Duration = Duration::from_millis(1);

        let this = FileTypes::load::<_, FORMAT>().await?;
        let watched = WatchFile::<_, FORMAT> {
            data: Arc::new(RwLock::new(this)),
        };

        let fut = {
            let root = get_env_var("SHAKEN_DATA_DIR").map(PathBuf::from)?;
            let path = Self::get_path(&root);

            let watched = watched.clone();
            let callback = move |_| {
                let watched = watched.clone();
                async move {
                    let this = FileTypes::load::<_, FORMAT>().await?;
                    *watched.data.write().await = this;
                    Ok(())
                }
            };

            watch_file(path, SLEEP, MODIFICATION, callback)
        };

        tokio::spawn(fut);

        Ok(watched)
    }
}
