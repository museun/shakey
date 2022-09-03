use super::{load_yaml, save_yaml, Interest};

pub struct FileTypes;

impl FileTypes {
    pub const YAML: u8 = 1;

    pub async fn load<T, const FORMAT: u8>() -> anyhow::Result<T>
    where
        T: Interest + for<'de> serde::Deserialize<'de>,
    {
        match FORMAT {
            1 => load_yaml::<T>().await,
            _ => anyhow::bail!("unsupported format"),
        }
    }

    pub async fn save<T, const FORMAT: u8>(val: &T) -> anyhow::Result<()>
    where
        T: Interest + serde::Serialize + Send + Sync,
    {
        match FORMAT {
            1 => save_yaml(val).await,
            _ => anyhow::bail!("unsupported format"),
        }
    }
}
