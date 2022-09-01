#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use std::{future::Future, path::PathBuf, sync::Arc, time::Duration};

use shakey::{
    config::Config,
    data::{BoxedFuture, Interest},
    ext::FutureExt,
    global::{Global, GlobalItem},
    handler::{Bindable, BoxedCallable, Components},
    irc, Commands, Replier, Templates,
};
use tokio::{sync::Notify, task::JoinHandle};

async fn initialize<T>(
    stop: impl Future<Output = ()> + Send + 'static,
) -> anyhow::Result<JoinHandle<()>>
where
    T: Default + Send + Sync + 'static,
    T: Interest + for<'de> serde::Deserialize<'de>,
    Global<'static, T>:,
    T: GlobalItem,
{
    async fn reload<T>(path: PathBuf) -> anyhow::Result<()>
    where
        T: Default + Send + Sync + 'static,
        T: Interest + for<'de> serde::Deserialize<'de>,
        Global<'static, T>:,
        T: GlobalItem,
    {
        let this = shakey::data::load_yaml().await?;
        T::get_static().reset();
        T::get_static().initialize(Arc::new(this));
        Ok(())
    }

    let path = shakey::data::get_data_path::<T>()?;
    reload::<T>(path.clone()).await?;

    Ok(tokio::spawn(async move {
        let fut = shakey::watch_file(
            path,
            Duration::from_secs(1),
            Duration::from_millis(1),
            reload::<T>,
        );

        use shakey::ext::Either::*;
        match stop.select(fut).await {
            Left(..) => {}
            Right(Err(err)) => log::error!("could not reload {}: {err}", T::description()),
            _ => {}
        }
    }))
}

struct Modules<'a, R: Replier> {
    components: &'a Components,
    inner: Vec<BoxedCallable<R>>,
}

impl<'a, R: Replier> Modules<'a, R> {
    fn new(components: &'a Components) -> Self {
        Self {
            components,
            inner: vec![],
        }
    }

    async fn add<T: Bindable<R>>(mut self) -> anyhow::Result<Modules<'a, R>> {
        let binding = T::bind(self.components).await?;
        self.inner.push(binding.into_callable());
        Ok(self)
    }

    fn into_list(self) -> Vec<BoxedCallable<R>> {
        self.inner
    }
}

#[rustfmt::skip]
async fn bind_modules<R: Replier>(components: & Components) -> anyhow::Result<Vec<BoxedCallable<R>>> {
    use shakey::modules::*;
    Ok(Modules::<R>::new(components)
        .add::<Builtin>().await?
        .add::<Twitch>().await?
        .add::<Spotify>().await?
        .add::<Crates>().await?
        .add::<Vscode>().await?
        .add::<Help>().await?
        .add::<UserDefined>().await?
        .add::<AnotherViewer>().await?
        .add::<Shakespeare>().await?
        .into_list())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from([".dev.env", ".secrets.env"]);
    alto_logger::init_term_logger()?;

    let config = Config::load("config.yaml").await?;
    let components = shakey::register_components(&config).await?;

    loop {
        let (notified, notifier) = notify();

        let commands_task = initialize::<Commands>(notified()).await?;
        let templates_task = initialize::<Templates>(notified()).await?;

        let modules = bind_modules(&components).await?;

        if let Err(err) = async move {
            shakey::irc::run(modules).await?;
            anyhow::Result::<_, anyhow::Error>::Ok(())
        }
        .await
        {
            log::warn!("disconnected");
            match () {
                _ if err.is::<irc::errors::Connection>() => {}
                _ if err.is::<irc::errors::Eof>() => {}
                _ if err.is::<irc::errors::Timeout>() => {}
                _ => {
                    log::error!("{err}");
                    std::process::exit(1)
                }
            }

            notifier();
            let _ = tokio::join!(commands_task, templates_task,);

            log::warn!("reconnecting in 5 seconds");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}

fn notify() -> (impl Fn() -> BoxedFuture<'static, ()>, impl FnOnce()) {
    let notify = Arc::new(Notify::new());
    (
        {
            let notify = notify.clone();
            move || {
                let notify = notify.clone();
                Box::pin(async move { notify.notified().await })
            }
        },
        move || notify.notify_waiters(),
    )
}

#[cfg(not(debug_assertions))]
const _: () = {
    include_str!("commands.yaml");
};

#[cfg(not(debug_assertions))]
const _: () = {
    include_str!("templates.yaml");
};

// BUG EXTRA TODO (really read this one) provide default commands/templates.yaml
