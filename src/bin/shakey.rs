#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use std::{future::Future, path::PathBuf, sync::Arc, time::Duration};

use shakey::{
    config::Config,
    data::Interest,
    ext::{Either, FutureExt},
    get_env_var,
    global::{Global, GlobalItem},
    handler::{Bindable, Components, SharedCallable},
    irc,
    templates::reset_registry,
    Commands, Replier, Templates,
};
use tokio::task::JoinHandle;

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
        let data = tokio::fs::read_to_string(&path).await?;
        let this = serde_yaml::from_str(&data)?;
        T::get_static().initialize(Arc::new(this));
        Ok(())
    }

    let config_root = get_env_var("SHAKEN_CONFIG_DIR").map(PathBuf::from)?;
    let path = T::get_path(&config_root);
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
    inner: Vec<SharedCallable<R>>,
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

    fn into_list(self) -> Vec<SharedCallable<R>> {
        self.inner
    }
}

async fn bind_modules<R: Replier>(
    components: &Components,
) -> anyhow::Result<Vec<SharedCallable<R>>> {
    use shakey::modules::*;

    reset_registry();
    Ok(Modules::<R>::new(components)
        .add::<Builtin>()
        .await?
        .add::<Twitch>()
        .await?
        .add::<Spotify>()
        .await?
        .add::<Crates>()
        .await?
        .add::<Vscode>()
        .await?
        .add::<Help>()
        .await?
        .add::<UserDefined>()
        .await?
        .add::<AnotherViewer>()
        .await?
        .add::<Shakespeare>()
        .await?
        .into_list())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from([".dev.env", ".secrets.env"]);

    alto_logger::init_alt_term_logger()?;

    let config = Config::load("config.yaml").await?;
    let components = shakey::handler::register_components(&config).await?;

    loop {
        let notify = Notify::new();

        let commands_task = initialize::<Commands>(notify.notifier()).await?;
        let templates_task = initialize::<Templates>(notify.notifier()).await?;

        let modules = bind_modules(&components).await?;

        // TODO don't do this in the loop
        // OR: shut it down before the next iteration
        let discord = tokio::spawn({
            let modules = modules.clone();
            let stop = notify.notifier();
            async move {
                match stop.select(shakey::twilight::run(modules)).await {
                    Either::Left(..) => return,
                    Either::Right(..) => return,
                }
            }
        });

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

            notify.notify().await;
            let _ = tokio::join!(commands_task, templates_task, discord);

            log::warn!("reconnecting in 5 seconds");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}

struct Notify {
    sender: flume::Sender<()>,
    recv: flume::Receiver<()>,
}

impl Notify {
    fn new() -> Self {
        let (sender, recv) = flume::bounded(0);
        Self { sender, recv }
    }

    fn notifier(&self) -> impl Future<Output = ()> {
        let recv = self.recv.clone();
        async move {
            let _ = recv.recv_async().await;
        }
    }

    async fn notify(self) {
        let _ = self.sender.into_send_async(()).await;
    }
}
