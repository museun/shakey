#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use std::sync::Arc;

use shakey::{get_env_var, irc, Commands, Templates};

async fn initialize_templates() -> anyhow::Result<()> {
    let path = get_env_var("SHAKEN_TEMPLATES_PATH")?;
    let templates = Templates::load_from_yaml(&path).await.map(Arc::new)?;
    shakey::global::GLOBAL_TEMPLATES.initialize(templates);
    shakey::bind_system_errors()?;

    tokio::spawn(async move {
        if let Err(err) = Templates::watch_for_updates(path).await {
            log::error!("could not reload templates: {err}");
            std::process::exit(0) // TODO not this
        }
    });

    Ok(())
}

async fn initialize_commands() -> anyhow::Result<()> {
    let path = get_env_var("SHAKEN_COMMANDS_PATH")?;
    let commands = Commands::load_from_yaml(&path).await.map(Arc::new)?;
    shakey::global::GLOBAL_COMMANDS.initialize(commands);

    tokio::spawn(async move {
        if let Err(err) = Commands::watch_for_updates(path).await {
            log::error!("could not reload commands: {err}");
            std::process::exit(0) // TODO not this
        }
    });

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from([".dev.env", ".secrets.env"]);
    alto_logger::init_term_logger()?;

    let helix_client_id = get_env_var("SHAKEN_TWITCH_CLIENT_ID")?;
    let helix_client_secret = get_env_var("SHAKEN_TWITCH_CLIENT_SECRET")?;

    let helix_oauth = shakey::helix::OAuth::create(&helix_client_id, &helix_client_secret).await?;
    let helix_client = shakey::helix::HelixClient::new(helix_oauth);

    loop {
        initialize_commands().await?;
        initialize_templates().await?;

        let builtin = builtin::Builtin::bind().await?.into_callable();
        let twitch = twitch::Twitch::bind(helix_client.clone())
            .await?
            .into_callable();

        if let Err(err) = async move {
            shakey::irc::run([
                builtin, //
                twitch,
            ])
            .await?;
            anyhow::Result::<_, anyhow::Error>::Ok(())
        }
        .await
        {
            log::warn!("disconnected");
            match () {
                _ if err.is::<irc::errors::Connection>() => {}
                _ if err.is::<irc::errors::Eof>() => {}
                _ if err.is::<irc::errors::Timeout>() => {}
                _ => {
                    log::error!("{err}");
                    std::process::exit(1)
                }
            }

            log::warn!("reconnecting in 5 seconds");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}

mod twitch {
    use shakey::{
        ext::FormatTime,
        helix::{data::Stream, HelixClient},
        irc::Message,
        Arguments, Bind, Replier,
    };
    use time::OffsetDateTime;

    shakey::make_response! {
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

    impl Twitch {
        pub async fn bind<R: Replier + 'static>(
            client: HelixClient,
        ) -> anyhow::Result<Bind<Self, R>> {
            Bind::create::<responses::Responses>(Self { client })?
                .bind(Self::uptime)?
                .bind(Self::viewers)
        }

        fn uptime(&mut self, msg: &Message<impl Replier>, args: Arguments) {
            let msg = msg.clone();
            let client = self.client.clone();

            tokio::spawn(async move {
                let Stream {
                    user_name: name,
                    started_at,
                    ..
                } = match Self::get_stream(&client, &msg, &args).await? {
                    Some(stream) => stream,
                    None => return anyhow::Result::<_, anyhow::Error>::Ok(()),
                };

                let uptime = (OffsetDateTime::now_utc() - started_at).as_readable_time();
                msg.say(responses::Uptime { name, uptime });

                anyhow::Result::Ok(())
            });
        }

        fn viewers(&mut self, msg: &Message<impl Replier>, args: Arguments) {
            let msg = msg.clone();
            let client = self.client.clone();

            tokio::spawn(async move {
                let Stream {
                    user_name: name,
                    viewer_count: viewers,
                    ..
                } = match Self::get_stream(&client, &msg, &args).await? {
                    Some(stream) => stream,
                    None => return anyhow::Result::<_, anyhow::Error>::Ok(()),
                };

                msg.say(responses::Viewers { name, viewers });

                anyhow::Result::Ok(())
            });
        }

        async fn get_stream(
            client: &HelixClient,
            msg: &Message<impl Replier>,
            args: &Arguments,
        ) -> anyhow::Result<Option<Stream>> {
            let channel = args.get("channel").unwrap_or_else(|| &msg.target);
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
}

mod builtin {
    use std::{borrow::Cow, time::Duration};

    use fastrand::Rng;
    use fastrand_ext::SliceExt;
    use shakey::{
        data::{Interest, Watch, WatchFileYaml},
        ext::FormatTime,
        irc::{Message, Replier},
        Arguments, Bind,
    };
    use time::{format_description::FormatItem, macros::format_description, OffsetDateTime};
    use tokio::time::Instant;

    shakey::make_response! {
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
        fn module() -> &'static str {
            "builtin"
        }
        fn file() -> &'static str {
            "greetings.yaml"
        }
    }

    pub struct Builtin {
        uptime: Instant,
        greetings: WatchFileYaml<Greetings>,
    }

    impl Builtin {
        pub async fn bind<R>() -> anyhow::Result<Bind<Self, R>>
        where
            R: Replier + 'static,
        {
            let greetings = Greetings::watch().await?;
            let this = Self {
                greetings,
                uptime: Instant::now(),
            };

            Bind::create::<responses::Responses>(this)?
                .bind(Self::ping)?
                .bind(Self::hello)?
                .bind(Self::time)?
                .bind(Self::bot_uptime)?
                .bind(Self::version)?
                .listen(Self::say_hello)
        }

        fn ping(&mut self, msg: &Message<impl Replier>, args: Arguments) -> anyhow::Result<()> {
            let now = OffsetDateTime::now_local()?;
            let ms: Duration = (now - msg.timestamp).try_into()?;
            let time = format!("{ms:.1?}");

            match args.get("token").map(ToString::to_string) {
                Some(token) => msg.say(responses::PingWithToken { token, time }),
                None => msg.say(responses::Ping { time }),
            }

            Ok(())
        }

        fn hello(&mut self, msg: &Message<impl Replier>, args: Arguments) {
            let msg = msg.clone();
            let greetings = self.greetings.clone();
            tokio::spawn(async move {
                let greetings = greetings.get().await;
                let greeting = greetings.choose(&fastrand::Rng::new());
                msg.say(responses::Hello {
                    greeting: greeting.to_string(),
                    sender: msg.sender.to_string(),
                })
            });
        }

        fn time(&mut self, msg: &Message<impl Replier>, args: Arguments) -> anyhow::Result<()> {
            static FMT: &[FormatItem<'static>] = format_description!("[hour]:[minute]:[second]");
            let now = OffsetDateTime::now_local()?.format(&FMT)?;
            msg.say(responses::Time { now });
            Ok(())
        }

        fn bot_uptime(&mut self, msg: &Message<impl Replier>, args: Arguments) {
            let uptime = self.uptime.elapsed().as_readable_time();
            msg.say(responses::BotUptime { uptime })
        }

        fn version(&mut self, msg: &Message<impl Replier>, args: Arguments) {
            msg.say(responses::Version {
                revision: shakey::GIT_REVISION.to_string(),
                branch: shakey::GIT_BRANCH.to_string(),
                build_time: shakey::BUILD_TIME.to_string(),
            });
        }

        fn say_hello(&mut self, msg: &Message<impl Replier>) {
            let msg = msg.clone();
            let greetings = self.greetings.clone();
            tokio::spawn(async move {
                let data = msg.data.trim_end_matches(['!', '?', '.']);
                let greetings = greetings.get().await;
                if greetings.contains(data) {
                    let greeting = greetings.choose(&fastrand::Rng::new());
                    msg.say(responses::SayHello {
                        greeting: greeting.to_string(),
                        sender: msg.sender.to_string(),
                    })
                }
            });
        }
    }
}
