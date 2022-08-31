#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use std::sync::Arc;

use shakey::{get_env_var, irc, Commands, Templates};

async fn initialize_templates() -> anyhow::Result<()> {
    let path = get_env_var("SHAKEN_TEMPLATES_PATH")?;
    let templates = Templates::load_from_yaml(&path).await.map(Arc::new)?;
    shakey::global::GLOBAL_TEMPLATES.initialize(templates);
    shakey::bind_system_errors()?;

    tokio::spawn(async move {
        if let Err(err) = Templates::watch_for_updates(path).await {
            log::error!("could not reload templates: {err}");
            std::process::exit(0) // TODO not this
        }
    });

    Ok(())
}

async fn initialize_commands() -> anyhow::Result<()> {
    let path = get_env_var("SHAKEN_COMMANDS_PATH")?;
    let commands = Commands::load_from_yaml(&path).await.map(Arc::new)?;
    shakey::global::GLOBAL_COMMANDS.initialize(commands);

    tokio::spawn(async move {
        if let Err(err) = Commands::watch_for_updates(path).await {
            log::error!("could not reload commands: {err}");
            std::process::exit(0) // TODO not this
        }
    });

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from([".dev.env", ".secrets.env"]);
    alto_logger::init_term_logger()?;

    let helix_client_id = get_env_var("SHAKEN_TWITCH_CLIENT_ID")?;
    let helix_client_secret = get_env_var("SHAKEN_TWITCH_CLIENT_SECRET")?;

    let helix_oauth = shakey::helix::OAuth::create(&helix_client_id, &helix_client_secret).await?;
    let helix_client = shakey::helix::HelixClient::new(helix_oauth);

    let spotify_client_id = get_env_var("SHAKEN_SPOTIFY_CLIENT_ID")?;
    let spotify_client_secret = get_env_var("SHAKEN_SPOTIFY_CLIENT_SECRET")?;
    let spotify_client =
        shakey::modules::SpotifyClient::new(&spotify_client_id, &spotify_client_secret).await?;

    let gist_id = get_env_var("SHAKEN_SETTINGS_GIST_ID")?;
    let gist_id = Arc::<str>::from(&*gist_id);

    let github_oauth_token = get_env_var("SHAKEN_GITHUB_OAUTH_TOKEN")?;
    let oauth = Arc::new(shakey::modules::GithubOAuth {
        token: github_oauth_token,
    });

    loop {
        initialize_commands().await?;
        initialize_templates().await?;

        let builtin = shakey::modules::Builtin::bind().await?.into_callable();
        let twitch = shakey::modules::Twitch::bind(helix_client.clone())
            .await?
            .into_callable();

        let spotify = shakey::modules::Spotify::bind(spotify_client.clone())
            .await?
            .into_callable();

        let crates = shakey::modules::Crates::bind().await?.into_callable();

        let vscode = shakey::modules::Vscode::bind(gist_id.clone(), oauth.clone())
            .await?
            .into_callable();

        let help = shakey::modules::Help::bind().await?.into_callable();

        let user_defined = shakey::modules::UserDefined::bind().await?.into_callable();

        if let Err(err) = async move {
            shakey::irc::run([
                builtin, //
                twitch,  //
                spotify, //
                crates,  //
                vscode,  //
                help,    //
                user_defined,
            ])
            .await?;
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
