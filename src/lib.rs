#[macro_use]
pub mod templates;

pub use templates::{BorrowedEnv, Environment, RegisterResponse, Templates};

pub mod handler;
pub use handler::{Arguments, Bind, Commands, MaybeTask, Outcome, Replier, Reply, Response};

pub mod env;

pub mod data;
pub mod ext;
pub mod global;
pub mod helix;
pub mod irc;
pub mod modules;

mod get_fields;
mod serde;

// mod testing;
// pub use testing::mock;

mod github;
mod spotify;

pub mod config;

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

    struct RequiresAdmin {
    } is "requires_admin"
}

pub fn bind_system_errors() -> anyhow::Result<()> {
    use crate::RegisterResponse as _;
    responses::Responses::register()
}

include!(concat!(env!("OUT_DIR"), "/", "version.rs"));

pub const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

mod message;
pub use message::Message;

pub mod twilight;
