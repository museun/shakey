use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

mod outcome;
pub use outcome::{MaybeTask, Outcome};

mod bind;
pub use bind::{Bind, Commands};

mod response;
pub use response::Response;

mod reply;
pub use reply::Reply;

mod arguments;
pub use arguments::Arguments;

mod replier;
pub use replier::Replier;

use crate::RegisterResponse;

#[derive(Default, Clone)]
pub struct Components {
    map: Arc<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl Components {
    pub fn register<T: Any + Send + Sync + 'static>(mut self, item: T) -> Self {
        assert!(Arc::get_mut(&mut self.map)
            .expect("single ownership")
            .insert(TypeId::of::<T>(), Box::new(item))
            .is_none());
        self
    }

    pub fn get_ref<T: Any + Send + Sync + 'static>(&self) -> &T {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|inner| inner.downcast_ref::<T>())
            .unwrap_or_else(|| {
                panic!(
                    "expected key `{}` to exist in components",
                    std::any::type_name::<T>()
                )
            })
    }

    pub fn get<T: Any + Send + Sync + 'static + Clone>(&self) -> T {
        self.get_ref::<T>().clone()
    }
}

pub async fn register_components(config: &crate::config::Config) -> anyhow::Result<Components> {
    use crate::github::GistClient;
    use crate::helix::{EmoteMap, HelixClient, OAuth};
    use crate::spotify::SpotifyClient;

    let helix_client = OAuth::create(
        &config.helix.client_id, //
        &config.helix.client_secret,
    )
    .await
    .map(HelixClient::new)?;

    let emote_map = helix_client
        .get_global_emotes()
        .await
        .map(|(_, map)| {
            map.iter()
                .map(|emote| (&*emote.name, &*emote.id))
                .fold(EmoteMap::default(), |map, (name, id)| {
                    map.with_emote(name, id)
                })
        })
        .map(Arc::new)?;

    let spotify_client = SpotifyClient::new(
        &config.spotify.client_id, //
        &config.spotify.client_secret,
    )
    .await?;

    let gist_client = GistClient::new(
        &config.github.oauth_token, //
    );

    Ok(Components::default() //
        .register(helix_client)
        .register(emote_map)
        .register(spotify_client)
        .register(gist_client))
}

#[async_trait::async_trait]
pub trait Bindable<R: Replier>: Sized + Send + Sync + 'static {
    type Responses: RegisterResponse;
    async fn bind(components: &Components) -> anyhow::Result<Bind<Self, R>>;
}

pub type SharedCallable<R = Box<dyn Response>> = Arc<dyn Fn(crate::Message<R>) + Send + Sync>;
