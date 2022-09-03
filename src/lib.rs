#[macro_use]
pub mod templates;

pub use templates::{BorrowedEnv, Environment, RegisterResponse, Templates};

pub mod handler;
pub use handler::{Arguments, Bind, Commands, MaybeTask, Outcome, Replier, Reply, Response};

pub mod env;

pub mod data;
pub mod ext;
pub mod global;
pub mod helix;
pub mod irc;
pub mod modules;

mod get_fields;
mod serde;

// mod testing;
// pub use testing::mock;

mod github;
mod spotify;

pub mod config;

crate::make_response! {
    module: "system"

    struct Error {
        error: String,
    } is "command_error"

    struct InvalidUsage {
        usage: String,
    } is "invalid_usage"

    struct RequiresPermission {
    } is "requires_permission"

    struct RequiresAdmin {
    } is "requires_admin"
}

pub fn bind_system_errors() -> anyhow::Result<()> {
    use crate::RegisterResponse as _;
    responses::Responses::register()
}

include!(concat!(env!("OUT_DIR"), "/", "version.rs"));

pub const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

mod message;
pub use message::Message;

pub mod twilight {
    use anyhow::Context;
    use std::{collections::HashMap, future::Future, sync::Arc};
    use time::OffsetDateTime;
    use tokio::sync::{mpsc::UnboundedReceiver, Mutex};
    use tokio_stream::StreamExt;
    use twilight_gateway::Shard;
    use twilight_http::Client;
    use twilight_model::{
        channel::message::MessageType,
        gateway::Intents,
        id::{
            marker::{ChannelMarker, MessageMarker},
            Id,
        },
    };

    use crate::{
        env::EnvVar, global::GlobalItem, handler::SharedCallable, Reply, Response, Templates,
    };

    #[derive(Clone)]
    pub struct Message {
        pub inner: Arc<twilight_model::channel::Message>,
        pub source: Arc<str>,
        pub timestamp: OffsetDateTime,
    }

    impl Message {
        fn new(inner: twilight_model::channel::Message, source: impl Into<Arc<str>>) -> Self {
            Self {
                inner: Arc::new(inner),
                source: source.into(),
                timestamp: time::OffsetDateTime::now_utc(),
            }
        }
    }

    impl std::ops::Deref for Message {
        type Target = twilight_model::channel::Message;
        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    pub async fn run(handlers: Vec<SharedCallable>) -> anyhow::Result<()> {
        let oauth_token = crate::env::SHAKEN_DISCORD_OAUTH_TOKEN::get()?;
        let client = Arc::new(twilight_http::Client::new(oauth_token.clone()));

        let (shard, mut events) = Shard::new(
            oauth_token,
            Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT,
        );
        shard.start().await?;

        let seen = DiscordState::default();
        let mut our_user_id = None;

        while let Some(event) = events.next().await {
            match event {
                twilight_gateway::Event::MessageCreate(msg)
                    if matches!(msg.kind, MessageType::Regular)
                        && Some(msg.author.id) != our_user_id =>
                {
                    let channel = seen
                        .channels
                        .update(msg.channel_id, {
                            let client = client.clone();
                            let id = msg.channel_id;
                            move || {
                                let client = client.clone();
                                async move { get_channel_name(&client, id).await }
                            }
                        })
                        .await?;

                    log::debug!("[{}] {}: {}", channel, msg.author.name, msg.content);

                    let (ch, id) = (msg.channel_id, msg.id);
                    let source = get_channel_name(&client, ch).await?;

                    let msg = Message::new(msg.0, source);

                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                    let msg = crate::Message::discord(msg, tx);

                    for handler in &handlers {
                        // outcome is always () here
                        (handler)(msg.clone());
                    }

                    tokio::spawn(read_responses(ch, id, rx, client.clone()));
                }
                twilight_gateway::Event::Ready(msg) => {
                    log::debug!("discord bot name: {}, id: {}", msg.user.name, msg.user.id);
                    our_user_id.get_or_insert(msg.user.id);
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn read_responses(
        ch_id: Id<ChannelMarker>,
        msg_id: Id<MessageMarker>,
        mut recv: UnboundedReceiver<Reply<Box<dyn Response>>>,
        client: Arc<Client>,
    ) {
        use crate::templates::Variant::Discord;
        while let Some(resp) = recv.recv().await {
            let resp = resp
                .map(|resp| Templates::get().render(&resp, Discord))
                .transpose();

            let resp = match resp {
                Some(inner) => inner,
                None => continue,
            };

            match resp {
                Reply::Say(resp) => {
                    if let Ok(ok) = client.create_message(ch_id).content(&resp) {
                        let _ = ok.exec().await;
                    }
                }
                Reply::Reply(resp) | Reply::Problem(resp) => {
                    if let Ok(ok) = client.create_message(ch_id).content(&resp) {
                        let _ = ok.reply(msg_id).exec().await;
                    }
                }
            }
        }
    }

    async fn get_channel_name(client: &Client, id: Id<ChannelMarker>) -> anyhow::Result<String> {
        let resp = client.channel(id).exec().await?;
        let name = resp
            .model()
            .await?
            .name
            .with_context(|| "cannot find name for {id}")?;
        Ok(name)
    }

    #[derive(Default)]
    struct DiscordState {
        channels: Map<ChannelMarker>,
    }

    #[derive(Clone)]
    struct Map<T, V = String> {
        map: Arc<Mutex<HashMap<Id<T>, Arc<V>>>>,
    }

    impl<T, V> Default for Map<T, V> {
        fn default() -> Self {
            Self {
                map: Default::default(),
            }
        }
    }

    impl<T> Map<T>
    where
        T: Send,
    {
        async fn update<S, Fut>(
            &self,
            id: Id<T>,
            vacant: impl Fn() -> Fut + Send + Sync,
        ) -> anyhow::Result<Arc<String>>
        where
            S: Into<String> + Send,
            Fut: Future<Output = anyhow::Result<S>> + Send,
        {
            use std::collections::hash_map::Entry;
            match self.map.lock().await.entry(id) {
                Entry::Occupied(t) => Ok(t.get().clone()),
                Entry::Vacant(t) => {
                    let data = vacant().await?.into();
                    Ok(t.insert(Arc::new(data)).clone())
                }
            }
        }
    }
}
