#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use std::{sync::Arc, time::Duration};

use shakey::{
    data::Interest,
    get_env_var,
    global::{Global, GlobalItem},
    helix::HelixClient,
    irc,
    modules::{GithubOAuth, SpotifyClient},
    Callable, Commands, Replier, Templates,
};

async fn initialize<T>() -> anyhow::Result<()>
where
    T: Default + Send + Sync + 'static,
    T: Interest + for<'de> serde::Deserialize<'de>,
    Global<'static, T>:,
    T: GlobalItem,
{
    let path = shakey::data::get_data_path::<T>()?;
    let this = shakey::data::load_yaml().await?;

    T::get_static().reset();
    T::get_static().initialize(Arc::new(this));

    tokio::spawn(async move {
        let fut = shakey::watch_file(
            path,
            Duration::from_secs(1),
            Duration::from_millis(1),
            move |path| async move {
                let this = shakey::data::load_yaml().await?;
                T::get_static().reset();
                T::get_static().initialize(Arc::new(this));
                Ok(())
            },
        );

        if let Err(err) = fut.await {
            log::error!("could not reload <replace me>: {err}");
            std::process::exit(0) // TODO not this
        }
    });

    Ok(())
}

#[derive(Clone)]
struct Components {
    helix_client: HelixClient,
    spotify_client: SpotifyClient,
    github_oauth: Arc<GithubOAuth>,
    gist_id: Arc<str>,
}

impl Components {
    async fn build() -> anyhow::Result<Self> {
        let helix_client_id = get_env_var("SHAKEN_TWITCH_CLIENT_ID")?;
        let helix_client_secret = get_env_var("SHAKEN_TWITCH_CLIENT_SECRET")?;

        let helix_oauth =
            shakey::helix::OAuth::create(&helix_client_id, &helix_client_secret).await?;
        let helix_client = shakey::helix::HelixClient::new(helix_oauth);

        let spotify_client_id = get_env_var("SHAKEN_SPOTIFY_CLIENT_ID")?;
        let spotify_client_secret = get_env_var("SHAKEN_SPOTIFY_CLIENT_SECRET")?;
        let spotify_client =
            shakey::modules::SpotifyClient::new(&spotify_client_id, &spotify_client_secret).await?;

        let gist_id = get_env_var("SHAKEN_SETTINGS_GIST_ID")?;
        let gist_id = Arc::<str>::from(&*gist_id);

        let github_oauth_token = get_env_var("SHAKEN_GITHUB_OAUTH_TOKEN")?;
        let github_oauth = Arc::new(shakey::modules::GithubOAuth {
            token: github_oauth_token,
        });

        Ok(Self {
            helix_client,
            spotify_client,
            github_oauth,
            gist_id,
        })
    }
}

async fn bind_modules<R: Replier>(
    components: Components,
) -> anyhow::Result<[Box<dyn Callable<irc::Message<R>, Outcome = ()>>; 7]> {
    use shakey::modules::*;

    let Components {
        helix_client,
        spotify_client,
        github_oauth,
        gist_id,
    } = components;

    let builtin = Builtin::bind().await?.into_callable();
    let twitch = Twitch::bind(helix_client).await?.into_callable();
    let spotify = Spotify::bind(spotify_client).await?.into_callable();
    let crates = Crates::bind().await?.into_callable();
    let vscode = Vscode::bind(gist_id, github_oauth).await?.into_callable();
    let help = Help::bind().await?.into_callable();
    let user_defined = UserDefined::bind().await?.into_callable();

    Ok([builtin, twitch, spotify, crates, vscode, help, user_defined])
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from([".dev.env", ".secrets.env"]);
    alto_logger::init_term_logger()?;

    let components = Components::build().await?;

    loop {
        initialize::<Commands>().await?;
        initialize::<Templates>().await?;

        let modules = bind_modules(components.clone()).await?;

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

            log::warn!("reconnecting in 5 seconds");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}
