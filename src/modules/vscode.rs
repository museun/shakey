use std::{collections::HashMap, sync::Arc};

use crate::{handler::Components, irc::Message, Arguments, Bind, Outcome, Replier};
use anyhow::Context;

crate::make_response! {
    module: "vscode"

    struct Theme {
        theme_url: String,
        variant: String,
    } is "theme"

    struct Fonts {
        editor: String,
        terminal: String,
    } is "fonts"
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FontsAndTheme {
    editor_font: String,
    terminal_font: String,
    theme_url: String,
    theme_variant: String,
}

pub struct OAuth {
    pub token: String,
}

pub struct Vscode {
    gist_id: Arc<str>,
    oauth: Arc<OAuth>,
}

impl Vscode {
    pub async fn bind<R: Replier>(components: Components) -> anyhow::Result<Bind<Self, R>> {
        Bind::create::<responses::Responses>(Self {
            gist_id: components.gist_id,
            oauth: components.github_oauth,
        })?
        .bind(Self::theme)?
        .bind(Self::fonts)
    }

    fn theme(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        async fn theme(
            msg: Message<impl Replier>,
            gist_id: Arc<str>,
            oauth: Arc<OAuth>,
        ) -> anyhow::Result<()> {
            let FontsAndTheme {
                theme_url,
                theme_variant,
                ..
            } = Vscode::get_current_settings(gist_id, oauth).await?;

            msg.say(responses::Theme {
                theme_url,
                variant: theme_variant,
            });

            Ok(())
        }

        let msg = msg.clone();
        let (gist_id, oauth) = (self.gist_id.clone(), self.oauth.clone());
        tokio::spawn(theme(msg, gist_id, oauth))
    }

    fn fonts(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        async fn fonts(
            msg: Message<impl Replier>,
            gist_id: Arc<str>,
            oauth: Arc<OAuth>,
        ) -> anyhow::Result<()> {
            let FontsAndTheme {
                editor_font,
                terminal_font,
                ..
            } = Vscode::get_current_settings(gist_id, oauth).await?;

            msg.say(responses::Fonts {
                editor: editor_font,
                terminal: terminal_font,
            });

            Ok(())
        }

        let msg = msg.clone();
        let (gist_id, oauth) = (self.gist_id.clone(), self.oauth.clone());
        tokio::spawn(fonts(msg, gist_id, oauth))
    }

    async fn get_current_settings(
        gist_id: Arc<str>,
        oauth: Arc<OAuth>,
    ) -> anyhow::Result<FontsAndTheme> {
        #[derive(Debug, ::serde::Deserialize)]
        struct File {
            content: String,
        }

        async fn get_gist_files(
            id: &str,
            OAuth { token }: &OAuth,
        ) -> anyhow::Result<HashMap<String, File>> {
            #[derive(Debug, ::serde::Deserialize)]
            struct Response {
                files: HashMap<String, File>,
            }

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

        let files = get_gist_files(&gist_id, &oauth).await?;
        let file = files
            .get("vscode settings.json") // TODO don't hardcode this
            .with_context(|| "cannot find settings")?;
        serde_json::from_str(&file.content).map_err(Into::into)
    }
}
