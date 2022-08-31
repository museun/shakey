pub mod handler;

use config::Secret;
pub use handler::{
    Arguments, Bind, Callable, Commands, MaybeTask, Outcome, Replier, Reply, Response,
};

#[macro_use]
pub mod templates;
pub use templates::{BorrowedEnv, Environment, RegisterResponse, Templates};

pub mod config;
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

#[derive(::serde::Deserialize)]
pub struct Config {
    pub helix_client_id: Secret,
    pub helix_client_secret: Secret,

    pub spotify_client_id: Secret,
    pub spotify_client_secret: Secret,

    pub settings_gist_id: Secret,
    pub github_oauth_token: Secret,
}

impl Config {
    pub async fn load(path: &str) -> anyhow::Result<Self> {
        let data = tokio::fs::read_to_string(path).await?;
        serde_yaml::from_str(&data).map_err(Into::into)
    }
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
