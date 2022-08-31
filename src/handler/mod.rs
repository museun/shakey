mod outcome;
use std::sync::Arc;

pub use outcome::{MaybeTask, Outcome};

mod callable;
pub use callable::Callable;

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

use crate::{helix::EmoteMap, Config};

#[derive(Clone)]
pub struct Components {
    pub helix_client: crate::helix::HelixClient,
    pub spotify_client: crate::modules::SpotifyClient,
    pub github_oauth: Arc<crate::modules::GithubOAuth>,
    pub gist_id: Arc<str>,
    pub emote_map: Arc<EmoteMap>,
}

impl Components {
    pub async fn build(config: &Config) -> anyhow::Result<Self> {
        use crate::helix::{HelixClient, OAuth};
        use crate::modules::{GithubOAuth, SpotifyClient};

        let helix_oauth = OAuth::create(
            &config.helix_client_id, //
            &config.helix_client_secret,
        )
        .await?;
        let helix_client = HelixClient::new(helix_oauth);

        let (_, emote_map) = helix_client.get_global_emotes().await?;
        let emote_map = EmoteMap::default()
            .with_emotes(emote_map.iter().map(|emote| (&*emote.name, &*emote.id)));

        let spotify_client = SpotifyClient::new(
            &config.spotify_client_id, //
            &config.spotify_client_secret,
        )
        .await?;

        let gist_id = Arc::<str>::from(&**config.settings_gist_id);
        let github_oauth = Arc::new(GithubOAuth {
            token: config.github_oauth_token.clone().into_value(),
        });

        Ok(Self {
            helix_client,
            spotify_client,
            github_oauth,
            gist_id,
            emote_map: Arc::new(emote_map),
        })
    }
}

pub type BoxedCallable<R = Box<dyn Response>> =
    Box<dyn Callable<crate::irc::Message<R>, Outcome = ()>>;
