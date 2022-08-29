#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

mod handler;
pub use handler::{Bind, Callable, Response};

mod templates;
pub use templates::{BorrowedEnv, Templates, Variant};

mod ext;
mod irc;
mod ser;
mod util;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    simple_env_load::load_env_from([".dev.env", ".secrets.env"]);

    if let Err(err) = async move {
        irc::run([], ["#museun"]).await?;
        anyhow::Result::<_, anyhow::Error>::Ok(())
    }
    .await
    {
        eprintln!("ERROR: {err}");
        std::process::exit(1)
    }
}

// &mut Foo::bind().await? as &mut dyn for<'i> Callable<irc::Message<'i>, Outcome = ()>
