use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::env::Secret;

#[derive(Clone)]
pub struct HelixClient {
    agent: reqwest::Client,
    oauth: Arc<OAuth>,
    base: Option<String>,
}

impl HelixClient {
    pub fn new(oauth: OAuth) -> Self {
        Self::new_with_ep(Option::<String>::None, oauth)
    }

    pub fn new_with_ep(ep: impl Into<Option<String>>, oauth: OAuth) -> Self {
        let agent = reqwest::Client::builder()
            .user_agent(crate::USER_AGENT)
            .build()
            .expect("valid client");

        Self {
            agent,
            oauth: Arc::new(oauth),
            base: ep.into().map(Into::into),
        }
    }

    pub async fn get_streams<const N: usize>(
        &self,
        names: [&str; N],
    ) -> anyhow::Result<Vec<data::Stream>> {
        self.get_response(
            "streams",
            &std::iter::repeat("user_login")
                .zip(names)
                .collect::<Vec<_>>(),
        )
        .await
        .map(|data| data.data)
    }

    pub async fn get_global_emotes(&self) -> anyhow::Result<(String, Vec<data::Emote>)> {
        self.get_response("chat/emotes/global", &[])
            .await
            .map(|data| (data.template, data.data))
    }

    pub async fn get_emotes_for(
        &self,
        broadcaster_id: &str,
    ) -> anyhow::Result<(String, Vec<data::Emote>)> {
        self.get_response("chat/emotes/global", &[("broadcaster_id", broadcaster_id)])
            .await
            .map(|data| (data.template, data.data))
    }

    async fn get_response<'k, 'v, T>(
        &self,
        ep: &str,
        query: &[(&'k str, &'v str)],
    ) -> anyhow::Result<data::Data<T>>
    where
        for<'de> T: ::serde::Deserialize<'de> + Send + 'static,
    {
        const BASE_URL: &str = "https://api.twitch.tv/helix";
        let url = format!("{}/{}", self.base.as_deref().unwrap_or(BASE_URL), ep);

        let response = [
            ("client-id", self.oauth.get_client_id()),
            ("authorization", self.oauth.get_bearer_token()),
        ]
        .into_iter()
        .fold(self.agent.get(&url), |req, (k, v)| req.header(k, v))
        .query(query)
        .header("User-Agent", crate::USER_AGENT)
        .send()
        .await?;

        Ok(response.json().await?)
    }
}

pub mod data {
    #[derive(::serde::Deserialize)]
    pub struct Data<T> {
        pub data: Vec<T>,
        #[serde(default)]
        pub template: String,
    }

    #[derive(Clone, Debug, ::serde::Deserialize)]
    pub struct Stream {
        #[serde(deserialize_with = "crate::serde::from_str")]
        pub id: u64,

        #[serde(deserialize_with = "crate::serde::from_str")]
        pub user_id: u64,
        pub user_name: String,

        #[serde(deserialize_with = "crate::serde::from_str")]
        pub game_id: u64,
        pub title: String,
        pub viewer_count: u64,

        #[serde(deserialize_with = "crate::serde::assume_utc_date_time")]
        pub started_at: time::OffsetDateTime,
    }

    #[derive(Debug, Clone, ::serde::Deserialize)]
    pub struct Emote {
        pub id: String,
        pub name: String,
    }
}

#[derive(Clone, Default)]
pub struct EmoteMap {
    name_to_id: HashMap<Box<str>, Box<str>>,
    id_to_name: HashMap<Box<str>, Box<str>>,
    names: HashSet<Box<str>>,
}

impl EmoteMap {
    pub fn with_emotes<'k, 'v, I>(self, iter: I) -> Self
    where
        I: Iterator<Item = (&'k str, &'v str)>,
    {
        iter.fold(self, |this, (name, id)| this.with_emote(name, id))
    }

    pub fn with_emote(mut self, name: &'_ str, id: &'_ str) -> Self {
        self.id_to_name.insert(id.into(), name.into());
        self.name_to_id.insert(name.into(), id.into());
        self.names.insert(name.into());
        self
    }

    pub fn get_name(&self, id: &str) -> Option<&str> {
        self.id_to_name.get(id).map(|s| &**s)
    }

    pub fn get_id(&self, name: &str) -> Option<&str> {
        self.name_to_id.get(name).map(|s| &**s)
    }

    pub fn has(&self, name: &str) -> bool {
        self.name_to_id.contains_key(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> + ExactSizeIterator + '_ {
        self.names.iter().map(|s| &**s)
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Config {
    pub client_id: Secret,
    pub client_secret: Secret,
}

#[derive(Clone, Debug, Default, ::serde::Deserialize)]
pub struct OAuth {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    pub token_type: String,

    #[serde(default)]
    client_id: String,

    #[serde(skip)]
    bearer_token: String,
}

impl OAuth {
    pub async fn create(client_id: &str, client_secret: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(!client_id.is_empty(), "twitch client id was empty");
        anyhow::ensure!(!client_secret.is_empty(), "twitch client secret was empty");

        let req = reqwest::Client::new()
            .post("https://id.twitch.tv/oauth2/token")
            .query(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("grant_type", "client_credentials"),
            ]);

        let resp = req.send().await?.json().await;
        Ok(resp.map(|this: Self| Self {
            client_id: client_id.to_string(),
            bearer_token: format!("Bearer {}", this.access_token),
            ..this
        })?)
    }

    pub fn with_client_credentials(client_id: &str, bearer_token: &str) -> Self {
        Self {
            client_id: client_id.to_string(),
            bearer_token: bearer_token.to_string(),
            ..Default::default()
        }
    }

    pub fn get_client_id(&self) -> &str {
        &self.client_id
    }

    pub fn get_bearer_token(&self) -> &str {
        &self.bearer_token
    }
}
