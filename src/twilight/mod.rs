use std::sync::Arc;

use anyhow::Context;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio_stream::StreamExt;

use {
    twilight_gateway::Shard,
    twilight_http::Client,
    twilight_model::{
        channel::message::MessageType,
        gateway::Intents,
        id::{
            marker::{ChannelMarker, MessageMarker},
            Id,
        },
    },
};

use crate::{env::EnvVar, global::GlobalItem, handler::SharedCallable, Reply, Response, Templates};

mod message;
pub use message::Message;

mod state;
use state::DiscordState;

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
