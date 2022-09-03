use std::{future::Future, path::PathBuf, sync::Arc, time::Duration};

use tokio::sync::{RwLock, RwLockMappedWriteGuard, RwLockReadGuard, RwLockWriteGuard};

use super::{FileTypes, Interest};

pub struct WatchFile<T, const FORMAT: u8 = { FileTypes::YAML }>
where
    T: Interest,
{
    pub(super) data: Arc<RwLock<T>>,
}

impl<T, const FORMAT: u8> WatchFile<T, FORMAT>
where
    T: Send + Sync,
    T: Interest,
{
    pub async fn get(&self) -> RwLockReadGuard<'_, T> {
        let g = self.data.read().await;
        RwLockReadGuard::map(g, |this| this)
    }

    pub async fn get_mut(&self) -> RwLockMappedWriteGuard<'_, T> {
        let g = self.data.write().await;
        RwLockWriteGuard::map(g, |this| &mut *this)
    }

    pub async fn save(&self) -> anyhow::Result<()>
    where
        T: serde::Serialize,
    {
        let this = self.data.read().await;
        FileTypes::save::<_, FORMAT>(&*this).await
    }
}

impl<T, const FORMAT: u8> Clone for WatchFile<T, FORMAT>
where
    T: Interest,
{
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}

pub async fn watch_file<Fut>(
    path: impl Into<PathBuf> + Send,
    sleep: Duration,
    modification: Duration,
    update: impl Fn(PathBuf) -> Fut + Sync + Send,
) -> anyhow::Result<()>
where
    Fut: Future<Output = anyhow::Result<()>> + Send,
{
    let path = path.into();

    let md = match tokio::fs::metadata(&path).await {
        Ok(md) => md,
        Err(err) => {
            log::error!("cannot read metadata for {}, {err}", path.display());
            anyhow::bail!("{err}")
        }
    };

    let mut last = md.modified()?;

    loop {
        tokio::time::sleep(sleep).await;

        let md = match tokio::fs::metadata(&path).await {
            Ok(md) => md,
            Err(err) => {
                log::error!("cannot read metadata for {}, {err}", path.display());
                continue;
            }
        };

        if md
            .modified()
            .ok()
            .and_then(|md| md.duration_since(last).ok())
            .filter(|&dur| dur >= modification)
            .is_some()
        {
            log::info!("file {} was modified", path.display());

            if let Err(err) = (update)(path.clone()).await {
                log::warn!("cannot update file: {err}");
                continue;
            }
            last = md
                .modified()
                .expect("already checked that the metadata exists")
        }
    }
}
