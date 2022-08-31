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

use crate::Config;

#[derive(Clone)]
pub struct Components {
    pub helix_client: crate::helix::HelixClient,
    pub spotify_client: crate::modules::SpotifyClient,
    pub github_oauth: Arc<crate::modules::GithubOAuth>,
    pub gist_id: Arc<str>,
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
        })
    }
}

pub type BoxedCallable = Box<dyn Callable<crate::irc::Message<Box<dyn Response>>, Outcome = ()>>;
