use crate::{
    ext::FormatTime,
    handler::{Bindable, Components},
    helix::{data::Stream, HelixClient},
    Arguments, Bind, Message, Outcome, Replier,
};
use time::OffsetDateTime;

crate::make_response! {
    module: "twitch"

    struct Viewers {
        name: String,
        viewers: u64
    } is "viewers"

    struct Uptime {
        name: String,
        uptime: String,
    } is "uptime"

    struct NotStreaming {
        channel: String,
    } is "not_streaming"
}

pub struct Twitch {
    client: HelixClient,
}

#[async_trait::async_trait]
impl<R: Replier> Bindable<R> for Twitch {
    type Responses = responses::Responses;

    async fn bind(components: &Components) -> anyhow::Result<Bind<Self, R>> {
        let this = Self {
            client: components.get(),
        };
        Bind::create(this)?.bind(Self::uptime)?.bind(Self::viewers)
    }
}

impl Twitch {
    fn uptime(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
        async fn uptime_async(
            client: HelixClient,
            msg: Message<impl Replier>,
            args: Arguments,
        ) -> anyhow::Result<()> {
            let Stream {
                user_name: name,
                started_at,
                ..
            } = match Twitch::get_stream(&client, &msg, &args).await? {
                Some(stream) => stream,
                None => return Ok(()),
            };

            let uptime = (OffsetDateTime::now_utc() - started_at).as_readable_time();
            msg.say(responses::Uptime { name, uptime });

            Ok(())
        }

        let msg = msg.clone();
        let client = self.client.clone();
        tokio::spawn(uptime_async(client, msg, args))
    }

    fn viewers(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
        async fn viewers(
            client: HelixClient,
            msg: Message<impl Replier>,
            args: Arguments,
        ) -> anyhow::Result<()> {
            let Stream {
                user_name: name,
                viewer_count: viewers,
                ..
            } = match Twitch::get_stream(&client, &msg, &args).await? {
                Some(stream) => stream,
                None => return Ok(()),
            };

            msg.say(responses::Viewers { name, viewers });

            Ok(())
        }

        let msg = msg.clone();
        let client = self.client.clone();
        tokio::spawn(viewers(client, msg, args))
    }

    async fn get_stream(
        client: &HelixClient,
        msg: &Message<impl Replier>,
        args: &Arguments,
    ) -> anyhow::Result<Option<Stream>> {
        let channel = args.get("channel").unwrap_or(&msg.target);
        let channel = channel.strip_prefix('#').unwrap_or(channel);

        if let Some(stream) = client.get_streams([channel]).await?.pop() {
            return Ok(Some(stream));
        }

        msg.problem(responses::NotStreaming {
            channel: channel.to_string(),
        });

        Ok(None)
    }
}
