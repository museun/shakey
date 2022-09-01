use crate::env::Secret;

#[derive(::serde::Deserialize)]
pub struct HelixConfig {
    pub client_id: Secret,
    pub client_secret: Secret,
}

#[derive(::serde::Deserialize)]
pub struct SpotifyConfig {
    pub client_id: Secret,
    pub client_secret: Secret,
}

#[derive(::serde::Deserialize)]
pub struct GithubConfig {
    pub oauth_token: Secret,
}

#[derive(::serde::Deserialize)]
pub struct Config {
    pub helix: HelixConfig,
    pub spotify: SpotifyConfig,
    pub github: GithubConfig,
}

impl Config {
    pub async fn load(path: &str) -> anyhow::Result<Self> {
        let data = tokio::fs::read_to_string(path).await?;
        serde_yaml::from_str(&data).map_err(Into::into)
    }
}
