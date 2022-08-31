use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use tokio::sync::{RwLock, RwLockMappedWriteGuard, RwLockReadGuard, RwLockWriteGuard};

use crate::{get_env_var, util::watch_file};

pub async fn save_yaml<T: Interest>(val: &T) -> anyhow::Result<()>
where
    T: serde::Serialize,
{
    let data = serde_yaml::to_string(val)?;
    let root = get_env_var("SHAKEN_DATA_DIR").map(PathBuf::from)?;
    tokio::fs::write(T::get_path(&root), data)
        .await
        .map_err(Into::into)
}

pub async fn save_json<T: Interest>(val: &T) -> anyhow::Result<()>
where
    T: serde::Serialize,
{
    let data = serde_json::to_string(val)?;
    let root = get_env_var("SHAKEN_DATA_DIR").map(PathBuf::from)?;
    tokio::fs::write(T::get_path(&root), data)
        .await
        .map_err(Into::into)
}

async fn load_data<T: Interest>() -> anyhow::Result<String> {
    let root = get_env_var("SHAKEN_DATA_DIR").map(PathBuf::from)?;
    tokio::fs::read_to_string(T::get_path(&root))
        .await
        .map_err(Into::into)
}

pub async fn load_yaml<T>() -> anyhow::Result<T>
where
    T: Interest + for<'de> serde::Deserialize<'de>,
{
    serde_yaml::from_str(&load_data::<T>().await?).map_err(Into::into)
}

pub async fn load_json<T>() -> anyhow::Result<T>
where
    T: Interest + for<'de> serde::Deserialize<'de>,
{
    serde_json::from_str(&load_data::<T>().await?).map_err(Into::into)
}

pub struct FileTypes;

impl FileTypes {
    pub const YAML: u8 = 1;
    pub const JSON: u8 = 2;

    pub async fn load<T, const FORMAT: u8>() -> anyhow::Result<T>
    where
        T: Interest + for<'de> serde::Deserialize<'de>,
    {
        match FORMAT {
            1 => load_yaml::<T>().await,
            2 => load_json::<T>().await,
            _ => anyhow::bail!("unsupported format"),
        }
    }

    pub async fn save<T, const FORMAT: u8>(val: &T) -> anyhow::Result<()>
    where
        T: Interest + serde::Serialize + Send,
    {
        match FORMAT {
            1 => save_yaml(val).await,
            2 => save_json(val).await,
            _ => anyhow::bail!("unsupported format"),
        }
    }
}

pub struct SaveFile<T: Interest, const FORMAT: u8 = { FileTypes::YAML }>(Arc<RwLock<T>>);

impl<T: Interest, const FORMAT: u8> SaveFile<T, FORMAT> {
    pub async fn save(&self) -> anyhow::Result<()>
    where
        T: serde::Serialize + Send,
    {
        let this = self.0.read().await;
        FileTypes::save::<_, FORMAT>(&*this).await
    }

    pub async fn get(&self) -> RwLockReadGuard<'_, T> {
        let g = self.0.read().await;
        RwLockReadGuard::map(g, |this| &*this)
    }

    pub async fn get_mut(&self) -> RwLockMappedWriteGuard<'_, T> {
        let g = self.0.write().await;
        RwLockWriteGuard::map(g, |this| &mut *this)
    }
}

impl<T: Interest, const FORMAT: u8> Clone for SaveFile<T, FORMAT> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

pub struct WatchFile<T: Interest, const FORMAT: u8 = { FileTypes::YAML }>(Arc<RwLock<T>>);

impl<T: Interest, const FORMAT: u8> WatchFile<T, FORMAT>
where
    T: Send + Sync,
{
    pub async fn get(&self) -> RwLockReadGuard<'_, T> {
        let g = self.0.read().await;
        RwLockReadGuard::map(g, |this| &*this)
    }

    pub async fn get_mut(&self) -> RwLockMappedWriteGuard<'_, T> {
        let g = self.0.write().await;
        RwLockWriteGuard::map(g, |this| &mut *this)
    }
}

impl<T: Interest, const FORMAT: u8> Clone for WatchFile<T, FORMAT> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

pub type BoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait Save<const FORMAT: u8>: Sized + Interest {
    type Fut: Future<Output = anyhow::Result<SaveFile<Self, FORMAT>>>;
    fn saveable() -> Self::Fut;
}

impl<T, const FORMAT: u8> Save<FORMAT> for T
where
    T: Interest + for<'de> serde::Deserialize<'de>,
    T: Send + Sync + 'static,
{
    type Fut = BoxedFuture<'static, anyhow::Result<SaveFile<Self, FORMAT>>>;

    fn saveable() -> Self::Fut {
        Box::pin(async move {
            let this = FileTypes::load::<_, FORMAT>().await?;
            let saved = SaveFile::<_, FORMAT>(Arc::new(RwLock::new(this)));

            Ok(saved)
        })
    }
}

pub trait Watch<const FORMAT: u8>: Sized + Interest {
    type Fut: Future<Output = anyhow::Result<WatchFile<Self, FORMAT>>>;
    fn watch() -> Self::Fut;
}

impl<T, const FORMAT: u8> Watch<FORMAT> for T
where
    T: Interest + for<'de> serde::Deserialize<'de>,
    T: Send + Sync + 'static,
{
    type Fut = BoxedFuture<'static, anyhow::Result<WatchFile<Self, FORMAT>>>;

    fn watch() -> Self::Fut {
        const SLEEP: Duration = Duration::from_secs(1);
        const MODIFICATION: Duration = Duration::from_millis(1);

        Box::pin(async move {
            let this = FileTypes::load::<_, FORMAT>().await?;
            let watched = WatchFile::<_, FORMAT>(Arc::new(RwLock::new(this)));

            let fut = {
                let root = get_env_var("SHAKEN_DATA_DIR").map(PathBuf::from)?;
                let path = Self::get_path(&root);

                let watched = watched.clone();
                watch_file(path, SLEEP, MODIFICATION, {
                    move |_| async move {
                        let this = FileTypes::load::<_, FORMAT>().await?;
                        *watched.0.write().await = this;
                        Ok(())
                    }
                })
            };

            tokio::spawn(fut);

            Ok(watched)
        })
    }
}

pub trait Interest {
    fn module() -> &'static str;
    fn file() -> &'static str;

    fn get_path(root: &Path) -> PathBuf {
        root.join(Self::module()).join(Self::file())
    }
}
