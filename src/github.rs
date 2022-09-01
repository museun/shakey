use std::{collections::HashMap, sync::Arc};

#[derive(Debug, ::serde::Deserialize)]
pub struct GistFile {
    pub content: String,
}

#[derive(Clone)]
pub struct GistClient {
    oauth_bearer_token: Arc<str>,
}

impl GistClient {
    pub fn new(oauth_bearer_token: &str) -> Self {
        Self {
            oauth_bearer_token: Arc::from(oauth_bearer_token),
        }
    }

    pub async fn get_gist_files(&self, id: &str) -> anyhow::Result<HashMap<String, GistFile>> {
        #[derive(Debug, ::serde::Deserialize)]
        struct Response {
            files: HashMap<String, GistFile>,
        }

        let token = &self.oauth_bearer_token;

        let resp: Response = [
            ("Accept", "application/vnd.github+json"),
            ("Authorization", &format!("token {token}")),
            ("User-Agent", crate::USER_AGENT),
        ]
        .into_iter()
        .fold(
            reqwest::Client::new().get(format!("https://api.github.com/gists/{id}")),
            |req, (k, v)| req.header(k, v),
        )
        .send()
        .await?
        .json()
        .await?;

        Ok(resp.files)
    }
}
