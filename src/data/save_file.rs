use std::sync::Arc;

use tokio::sync::{RwLock, RwLockMappedWriteGuard, RwLockReadGuard, RwLockWriteGuard};

use super::{FileTypes, Interest};

pub struct SaveFile<T, const FORMAT: u8 = { FileTypes::YAML }>
where
    T: Interest,
{
    pub(super) data: Arc<RwLock<T>>,
}

impl<T, const FORMAT: u8> SaveFile<T, FORMAT>
where
    T: Interest,
{
    pub async fn save(&self) -> anyhow::Result<()>
    where
        T: serde::Serialize + Send + Sync,
    {
        let this = self.data.read().await;
        FileTypes::save::<_, FORMAT>(&*this).await
    }

    pub async fn get(&self) -> RwLockReadGuard<'_, T>
    where
        T: Send + Sync,
    {
        let g = self.data.read().await;
        RwLockReadGuard::map(g, |this| this)
    }

    pub async fn get_mut(&self) -> RwLockMappedWriteGuard<'_, T>
    where
        T: Send + Sync,
    {
        let g = self.data.write().await;
        RwLockWriteGuard::map(g, |this| &mut *this)
    }
}

impl<T, const FORMAT: u8> Clone for SaveFile<T, FORMAT>
where
    T: Interest,
{
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
        }
    }
}
