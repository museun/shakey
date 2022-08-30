pub mod handler;
pub use handler::{
    Arguments, Bind, Callable, Commands, ExampleArgs, Outcome, Replier, Reply, Response,
};

#[macro_use]
pub mod templates;
pub use templates::{BorrowedEnv, Environment, RegisterResponse, Templates};

pub mod irc;

pub mod ext;

mod ser;

mod util;
pub use util::get_env_var;

mod testing;
pub use testing::mock;

pub mod data;

pub mod global;

crate::make_response! {
    module: "system"

    struct Error {
        error: String,
    } is "command_error"

    struct InvalidUsage {
        usage: String,
    } is "invalid_usage"
}

pub fn bind_system_errors() -> anyhow::Result<()> {
    use crate::RegisterResponse as _;
    responses::Responses::register()
}

include!(concat!(env!("OUT_DIR"), "/", "version.rs"));
