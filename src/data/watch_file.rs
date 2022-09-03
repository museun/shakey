use std::sync::Arc;

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
