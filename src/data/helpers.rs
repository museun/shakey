use std::path::{Path, PathBuf};

use super::Interest;
use crate::get_env_var;

pub fn get_data_path<T>() -> anyhow::Result<PathBuf>
where
    T: Interest,
{
    get_env_var("SHAKEN_DATA_DIR").map(PathBuf::from)
}

pub async fn save_yaml<T: Interest>(val: &T) -> anyhow::Result<()>
where
    T: serde::Serialize + Send + Sync,
{
    let root = get_env_var("SHAKEN_DATA_DIR").map(PathBuf::from)?;
    save_yaml_to(val, &root).await
}

pub async fn save_yaml_to<T: Interest>(val: &T, root: &Path) -> anyhow::Result<()>
where
    T: serde::Serialize + Send + Sync,
{
    let data = serde_yaml::to_string(val)?;
    tokio::fs::write(T::get_path(root), data).await?;
    Ok(())
}

pub async fn load_yaml<T>() -> anyhow::Result<T>
where
    T: Interest + for<'de> serde::Deserialize<'de>,
{
    load_yaml_from(&get_data_path::<T>()?).await
}

pub async fn load_yaml_from<T>(root: &Path) -> anyhow::Result<T>
where
    T: Interest + for<'de> serde::Deserialize<'de>,
{
    let data = tokio::fs::read_to_string(T::get_path(root)).await?;
    serde_yaml::from_str(&data).map_err(Into::into)
}
