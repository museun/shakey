use std::{sync::Arc, time::Duration};

use once_cell::sync::Lazy;
use regex::Regex;
use tokio::{sync::Mutex, time::Instant};

use crate::{
    data::{Interest, InterestPath, Watch, WatchFile},
    handler::{Bindable, Components},
    Arguments, Bind, Message, Outcome, Replier,
};

crate::make_response! {
    module: "shakespeare"

    struct Respond {
        data: String
    } is "respond"

    struct Toggle {
        old: bool,
        new: bool,
    } is "toggle"
}

pub struct Shakespeare {
    last: Arc<Mutex<Option<Instant>>>,
    config: WatchFile<Config>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Config {
    brain_address: Box<str>,
    min_words: usize,
    max_words: usize,
    chance: f32,
    #[serde(with = "crate::serde::simple_human_time")]
    cooldown: Duration,
    enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            brain_address: Box::from("http://localhost:10000"),
            min_words: 5,
            max_words: 10,
            chance: 0.30,
            cooldown: Duration::from_secs(60),
            enabled: false,
        }
    }
}

impl Interest for Config {
    fn module() -> InterestPath<&'static str> {
        InterestPath::Nested("shakespeare")
    }

    fn file() -> &'static str {
        "config.yaml"
    }
}

#[async_trait::async_trait]
impl<R: Replier> Bindable<R> for Shakespeare {
    type Responses = responses::Responses;
    async fn bind(_: &Components) -> anyhow::Result<Bind<Self, R>> {
        let config = Config::watch().await?;
        Bind::create(Self {
            last: <Arc<Mutex<Option<_>>>>::default(),
            config,
        })?
        .bind(Self::toggle)?
        .bind(Self::speak)?
        .listen(Self::listen)
    }
}

impl Shakespeare {
    fn toggle(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        let msg = msg.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            if !msg.require_broadcaster() {
                return Ok(());
            }

            {
                let mut config = config.get_mut().await;
                let new = !config.enabled;
                let old = std::mem::replace(&mut config.enabled, new);
                msg.reply(responses::Toggle { old, new });
            }
            config.save().await
        })
    }

    fn speak(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        let msg = msg.clone();
        let config = self.config.clone();
        tokio::spawn(async move {
            if !config.get().await.enabled {
                log::info!("tried to speak, but shakespeare isn't enabled");
                return;
            }

            match Self::generate(config).await {
                Ok(data) => msg.say(responses::Respond { data }),
                Err(err) => log::error!("cannot generate response: {err}"),
            }
        })
    }

    fn listen(&mut self, msg: &Message<impl Replier>) {
        if msg.data.starts_with('!') {
            return;
        }

        if self.try_mention(msg) {
            return;
        }

        let last = self.last.clone();
        let msg = msg.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let c = config.get().await;
            if !c.enabled {
                return;
            }

            if fastrand::f32() < c.chance {
                return;
            }

            let mut g = last.lock().await;
            if g.is_none() || g.filter(|last| last.elapsed() >= c.cooldown).is_some() {
                g.replace(Instant::now());
                drop(c);

                match Self::generate(config).await {
                    Ok(data) => msg.say(responses::Respond { data }),
                    Err(err) => log::error!("cannot generate response: {err}"),
                }
            }
        });
    }

    fn try_mention(&mut self, msg: &Message<impl Replier>) -> bool {
        static NAME_PATTERN: Lazy<Regex> =
            Lazy::new(|| Regex::new("^@?(?:shaken(?:_bot)?|shakey)(?:,.?!:)?").unwrap());

        if msg
            .data
            .split_ascii_whitespace()
            .any(|part| NAME_PATTERN.is_match(part))
        {
            let msg = msg.clone();
            let config = self.config.clone();
            let last = self.last.clone();
            tokio::spawn(async move {
                if !config.get().await.enabled {
                    return;
                }

                last.lock().await.replace(Instant::now());
                match Self::generate(config).await {
                    Ok(data) => msg.reply(responses::Respond { data }),
                    Err(err) => log::error!("cannot generate response: {err}"),
                }
            });
            return true;
        }

        false
    }

    async fn generate(config: WatchFile<Config>) -> anyhow::Result<String> {
        #[derive(serde::Serialize)]
        struct Query {
            min: usize,
            max: usize,
        }

        #[derive(serde::Deserialize)]
        struct Response {
            data: String,
        }

        let (url, query) = {
            let config = config.get().await;
            let url = format!("{}/generate", config.brain_address);
            let query = Query {
                min: config.min_words,
                max: config.max_words,
            };
            (url, query)
        };

        let resp: Response = reqwest::Client::new()
            .get(url)
            .json(&query)
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data)
    }
}
