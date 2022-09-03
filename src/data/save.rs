use std::sync::Arc;

use tokio::sync::RwLock;

use super::{FileTypes, Interest, SaveFile};

#[async_trait::async_trait]
pub trait Save<const FORMAT: u8>
where
    Self: Sized + Interest,
{
    async fn saveable() -> anyhow::Result<SaveFile<Self, FORMAT>>;
}

#[async_trait::async_trait]
impl<T, const FORMAT: u8> Save<FORMAT> for T
where
    T: Interest + for<'de> serde::Deserialize<'de>,
    T: Send + Sync + 'static,
{
    async fn saveable() -> anyhow::Result<SaveFile<Self, FORMAT>> {
        let this = FileTypes::load::<_, FORMAT>().await?;
        let saved = SaveFile::<_, FORMAT> {
            data: Arc::new(RwLock::new(this)),
        };
        Ok(saved)
    }
}
