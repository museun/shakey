use std::sync::Arc;

use crate::{
    get_env_var,
    github::GistClient,
    handler::{Bindable, Components},
    irc::Message,
    Arguments, Bind, Outcome, Replier,
};
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

pub struct Vscode {
    settings_gist_id: Arc<str>,
    gist_client: GistClient,
}

#[async_trait::async_trait]
impl<R: Replier> Bindable<R> for Vscode {
    type Responses = responses::Responses;

    async fn bind(components: &Components) -> anyhow::Result<Bind<Self, R>> {
        let this = Self {
            settings_gist_id: get_env_var("SHAKEN_SETTINGS_GIST_ID").map(Arc::from)?,
            gist_client: components.get(),
        };

        Bind::create(this)?.bind(Self::theme)?.bind(Self::fonts)
    }
}

impl Vscode {
    fn theme(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        async fn theme(
            msg: Message<impl Replier>,
            gist_id: Arc<str>,
            client: GistClient,
        ) -> anyhow::Result<()> {
            let FontsAndTheme {
                theme_url,
                theme_variant,
                ..
            } = Vscode::get_current_settings(gist_id, client).await?;

            msg.say(responses::Theme {
                theme_url,
                variant: theme_variant,
            });

            Ok(())
        }

        let msg = msg.clone();
        let (gist_id, client) = (self.settings_gist_id.clone(), self.gist_client.clone());
        tokio::spawn(theme(msg, gist_id, client))
    }

    fn fonts(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        async fn fonts(
            msg: Message<impl Replier>,
            gist_id: Arc<str>,
            client: GistClient,
        ) -> anyhow::Result<()> {
            let FontsAndTheme {
                editor_font,
                terminal_font,
                ..
            } = Vscode::get_current_settings(gist_id, client).await?;

            msg.say(responses::Fonts {
                editor: editor_font,
                terminal: terminal_font,
            });

            Ok(())
        }

        let msg = msg.clone();
        let (gist_id, client) = (self.settings_gist_id.clone(), self.gist_client.clone());
        tokio::spawn(fonts(msg, gist_id, client))
    }

    async fn get_current_settings(
        gist_id: Arc<str>,
        client: GistClient,
    ) -> anyhow::Result<FontsAndTheme> {
        let files = client.get_gist_files(&gist_id).await?;
        let file = files
            .get("vscode settings.json") // TODO don't hardcode this
            .with_context(|| "cannot find settings")?;
        serde_json::from_str(&file.content).map_err(Into::into)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FontsAndTheme {
    editor_font: String,
    terminal_font: String,
    theme_url: String,
    theme_variant: String,
}
