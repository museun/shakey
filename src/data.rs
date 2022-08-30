use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use tokio::sync::{RwLock, RwLockReadGuard};

use crate::{get_env_var, util::watch_file};

pub async fn load_yaml<T>() -> anyhow::Result<T>
where
    T: Interest + for<'de> serde::Deserialize<'de>,
{
    let root = get_env_var("SHAKEN_DATA_DIR").map(PathBuf::from)?;
    let data = tokio::fs::read_to_string(T::get_path(&root)).await?;
    serde_yaml::from_str(&data).map_err(Into::into)
}

pub struct WatchFileYaml<T>(Arc<RwLock<Arc<T>>>);

impl<T> WatchFileYaml<T> {
    pub async fn get(&self) -> RwLockReadGuard<'_, T> {
        let g = self.0.read().await;
        RwLockReadGuard::map(g, |this| &**this)
    }

    pub async fn get_owned(&self) -> Arc<T> {
        Arc::clone(&*self.0.read().await)
    }
}

impl<T> Clone for WatchFileYaml<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

pub type BoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait Watch: Sized {
    type Fut: Future<Output = anyhow::Result<WatchFileYaml<Self>>>;
    fn watch() -> Self::Fut;
}

impl<T> Watch for T
where
    T: Interest + for<'de> serde::Deserialize<'de>,
    T: Send + Sync + 'static,
{
    type Fut = BoxedFuture<'static, anyhow::Result<WatchFileYaml<Self>>>;

    fn watch() -> Self::Fut {
        const SLEEP: Duration = Duration::from_secs(1);
        const MODIFICATION: Duration = Duration::from_millis(1);

        Box::pin(async move {
            let root = get_env_var("SHAKEN_DATA_DIR").map(PathBuf::from)?;
            let path = Self::get_path(&root);

            let this = load_yaml().await?;
            let watched = WatchFileYaml(Arc::new(RwLock::new(Arc::new(this))));

            tokio::spawn({
                let watched = watched.clone();
                async move {
                    let _ = watch_file(path, SLEEP, MODIFICATION, {
                        let watched = watched.clone();
                        move |_| async move {
                            let this = load_yaml().await?;
                            *watched.0.write().await = Arc::new(this);
                            Ok(())
                        }
                    })
                    .await;
                }
            });

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
