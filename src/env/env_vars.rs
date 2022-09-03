pub trait EnvVar {
    fn key() -> &'static str;
    fn get() -> anyhow::Result<String>;
}

fn get_env_var<T: EnvVar>() -> anyhow::Result<String> {
    let key = T::key();
    log::trace!("loading: {key}");
    std::env::var(key).map_err(|_| anyhow::anyhow!("expected '{key}' to exist in env"))
}

macro_rules! make_env_key {
    ($($(#[$meta:meta])* $lit:ident)*) => {
        $(
            #[allow(non_camel_case_types)]
            #[derive(Copy, Clone)]
            $(#[$meta])*
            pub struct $lit;

            impl EnvVar for $lit {
                fn key() -> &'static str { stringify!($lit) }
                fn get() -> anyhow::Result<String> { get_env_var::<Self>() }
            }
        )*
    };
}

make_env_key! {
    /// Directory where to store data
    SHAKEN_DATA_DIR
    /// Directory where to store configurations
    SHAKEN_CONFIG_DIR

    /// The name of the bot for Twitch
    SHAKEN_TWITCH_NAME
    /// The address of the Twitch server
    SHAKEN_TWITCH_ADDRESS
    /// Comma-separated list of channels to join
    SHAKEN_TWITCH_CHANNELS
    /// Oauth token for using Twitch's API
    SHAKEN_TWITCH_OAUTH_TOKEN
    /// Twitch client id
    SHAKEN_TWITCH_CLIENT_ID
    /// Twitch client secret
    SHAKEN_TWITCH_CLIENT_SECRET

    /// OAuth token for connecting to Discord
    SHAKEN_DISCORD_OAUTH_TOKEN

    /// Spotify client id
    SHAKEN_SPOTIFY_CLIENT_ID
    /// Spotify client secret
    SHAKEN_SPOTIFY_CLIENT_SECRET

    /// GitHub oauth token
    SHAKEN_GITHUB_OAUTH_TOKEN
    /// VsCode settings gist id
    SHAKEN_SETTINGS_GIST_ID

    /// Url for the brain server
    SHAKEN_BRAIN_GENERATE_TOKEN
    /// Bearer token for write-access for the brain server
    SHAKEN_BRAIN_BEARER_TOKEN
    /// Bearer token for write-access for the what-song server
    SHAKEN_WHAT_SONG_BEARER_TOKEN

    /// Youtube API key
    SHAKEN_YOUTUBE_API_KEY

    RSPOTIFY_REDIRECT_URI
    RSPOTIFY_TOKEN_CACHE_FILE
}
