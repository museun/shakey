use std::{collections::HashMap, future::Future, sync::Arc};
use tokio::sync::Mutex;

use twilight_model::id::{marker::ChannelMarker, Id};

#[derive(Default)]
pub struct DiscordState {
    pub channels: Map<ChannelMarker>,
}

#[derive(Clone)]
pub struct Map<T, V = String> {
    map: Arc<Mutex<HashMap<Id<T>, Arc<V>>>>,
}

impl<T, V> Default for Map<T, V> {
    fn default() -> Self {
        Self {
            map: Default::default(),
        }
    }
}

impl<T> Map<T>
where
    T: Send,
{
    pub async fn update<S, Fut>(
        &self,
        id: Id<T>,
        vacant: impl Fn() -> Fut + Send + Sync,
    ) -> anyhow::Result<Arc<String>>
    where
        S: Into<String> + Send,
        Fut: Future<Output = anyhow::Result<S>> + Send,
    {
        use std::collections::hash_map::Entry;
        match self.map.lock().await.entry(id) {
            Entry::Occupied(t) => Ok(t.get().clone()),
            Entry::Vacant(t) => {
                let data = vacant().await?.into();
                Ok(t.insert(Arc::new(data)).clone())
            }
        }
    }
}
