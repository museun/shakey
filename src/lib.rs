#[macro_use]
pub mod templates;

use handler::Components;
pub use templates::{BorrowedEnv, Environment, RegisterResponse, Templates};

pub mod handler;
pub use handler::{
    Arguments, Bind, Callable, Commands, MaybeTask, Outcome, Replier, Reply, Response,
};

pub mod env;

pub mod data;
pub mod ext;
pub mod global;
pub mod helix;
pub mod irc;
pub mod modules;

mod ser;
mod serde;

mod util;
pub use util::{get_env_var, watch_file};

mod testing;
pub use testing::mock;

mod github;
mod spotify;

pub mod config;

pub async fn register_components(config: &crate::config::Config) -> anyhow::Result<Components> {
    use crate::github::GistClient;
    use crate::helix::{EmoteMap, HelixClient, OAuth};
    use crate::spotify::SpotifyClient;
    use std::sync::Arc;

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

crate::make_response! {
    module: "system"

    struct Error {
        error: String,
    } is "command_error"

    struct InvalidUsage {
        usage: String,
    } is "invalid_usage"

    struct RequiresPermission {
    } is "requires_permission"
}

pub fn bind_system_errors() -> anyhow::Result<()> {
    use crate::RegisterResponse as _;
    responses::Responses::register()
}

include!(concat!(env!("OUT_DIR"), "/", "version.rs"));

pub const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
