use std::{borrow::Cow, time::Duration};

use crate::{
    data::{FileTypes, Interest, Watch, WatchFile},
    ext::FormatTime,
    handler::{Bindable, Components},
    irc::Message,
    Arguments, Bind, Outcome, Replier,
};
use fastrand::Rng;
use fastrand_ext::SliceExt;
use time::{format_description::FormatItem, macros::format_description, OffsetDateTime};
use tokio::time::Instant;

crate::make_response! {
    module: "builtin"

    struct Ping {
        time: String
    } is "ping"

    struct PingWithToken {
        time: String,
        token: String
    } is "ping_with_token"

    struct Hello {
        greeting: String,
        sender: String
    } is "hello"

    struct Time {
        now: String,
    } is "time"

    struct BotUptime {
        uptime: String,
    } is "bot_uptime"

    struct Version {
        revision: String,
        branch: String,
        build_time: String,
    } is "version"

    struct SayHello {
        greeting: String,
        sender: String
    } is "say_hello"
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct Greetings(Vec<String>);

impl Greetings {
    fn contains(&self, key: &str) -> bool {
        self.0.iter().any(|s| s.eq_ignore_ascii_case(key))
    }

    fn choose(&self, rng: &Rng) -> Cow<'_, str> {
        (!self.0.is_empty())
            .then(|| self.0.choose(rng).map(|s| Cow::from(&**s)))
            .flatten()
            .unwrap_or(Cow::Borrowed("hello"))
    }
}

impl Interest for Greetings {
    fn module() -> Option<&'static str> {
        Some("builtin")
    }
    fn file() -> &'static str {
        "greetings.yaml"
    }
}

pub struct Builtin {
    uptime: Instant,
    greetings: WatchFile<Greetings>,
}

#[async_trait::async_trait]
impl<R: Replier> Bindable<R> for Builtin {
    type Responses = responses::Responses;
    async fn bind(_: &Components) -> anyhow::Result<Bind<Self, R>> {
        let greetings = Greetings::watch().await?;
        let this = Self {
            greetings,
            uptime: Instant::now(),
        };
        Bind::create(this)?
            .bind(Self::ping)?
            .bind(Self::hello)?
            .bind(Self::time)?
            .bind(Self::bot_uptime)?
            .bind(Self::version)?
            .listen(Self::say_hello)
    }
}

impl Builtin {
    fn ping(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
        let now = OffsetDateTime::now_local()?;
        let ms: Duration = (now - msg.timestamp).try_into()?;
        let time = format!("{ms:.1?}");

        match args.get("token").map(ToString::to_string) {
            Some(token) => msg.say(responses::PingWithToken { token, time }),
            None => msg.say(responses::Ping { time }),
        }

        Ok(())
    }

    fn hello(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        async fn hello(msg: Message<impl Replier>, greetings: WatchFile<Greetings>) {
            let greetings = greetings.get().await;
            let greeting = greetings.choose(&fastrand::Rng::new());
            msg.say(responses::Hello {
                greeting: greeting.to_string(),
                sender: msg.sender.to_string(),
            })
        }
        let msg = msg.clone();
        let greetings = self.greetings.clone();
        tokio::spawn(hello(msg, greetings))
    }

    fn time(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        static FMT: &[FormatItem<'static>] = format_description!("[hour]:[minute]:[second]");
        let now = OffsetDateTime::now_local()?.format(&FMT)?;
        msg.say(responses::Time { now });
        Ok(())
    }

    fn bot_uptime(&mut self, msg: &Message<impl Replier>, _: Arguments) {
        let uptime = self.uptime.elapsed().as_readable_time();
        msg.say(responses::BotUptime { uptime })
    }

    fn version(&mut self, msg: &Message<impl Replier>, _: Arguments) {
        msg.say(responses::Version {
            revision: crate::GIT_REVISION.to_string(),
            branch: crate::GIT_BRANCH.to_string(),
            build_time: crate::BUILD_TIME.to_string(),
        })
    }

    fn say_hello(&mut self, msg: &Message<impl Replier>) -> impl Outcome {
        async fn say_hello(
            msg: Message<impl Replier>,
            greetings: WatchFile<Greetings, { FileTypes::YAML }>,
        ) {
            let data = msg.data.trim_end_matches(['!', '?', '.']);
            let greetings = greetings.get().await;
            if !greetings.contains(data) {
                return;
            }

            let greeting = greetings.choose(&fastrand::Rng::new());
            msg.say(responses::SayHello {
                greeting: greeting.to_string(),
                sender: msg.sender.to_string(),
            })
        }

        let msg = msg.clone();
        let greetings = self.greetings.clone();
        tokio::spawn(say_hello(msg, greetings))
    }
}
