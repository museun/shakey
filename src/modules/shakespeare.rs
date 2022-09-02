use std::time::Duration;

use once_cell::sync::Lazy;
use regex::Regex;
use tokio::time::Instant;

use crate::{
    handler::{Bindable, Components},
    irc::Message,
    Arguments, Bind, Outcome, Replier,
};

crate::make_response! {
    module: "shakespeare"

    struct Respond {
        data: String
    } is "respond"
}

pub struct Shakespeare {
    last: Option<Instant>,
    chance: f32,
    cooldown: Duration,
}

#[async_trait::async_trait]
impl<R: Replier> Bindable<R> for Shakespeare {
    type Responses = responses::Responses;
    async fn bind(_: &Components) -> anyhow::Result<Bind<Self, R>> {
        Bind::create(Self {
            last: None,
            chance: 0.30,
            cooldown: Duration::from_secs(30),
        })?
        .bind(Self::speak)?
        .listen(Self::listen)
    }
}

impl Shakespeare {
    fn speak(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        let msg = msg.clone();
        tokio::spawn(async move {
            match Self::generate().await {
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

        if fastrand::f32() < self.chance {
            return;
        }

        if self.last.is_none()
            || self
                .last
                .filter(|last| last.elapsed() >= self.cooldown)
                .is_some()
        {
            self.last.replace(Instant::now());
            let msg = msg.clone();
            tokio::spawn(async move {
                match Self::generate().await {
                    Ok(data) => msg.say(responses::Respond { data }),
                    Err(err) => log::error!("cannot generate response: {err}"),
                }
            });
        }
    }

    fn try_mention(&mut self, msg: &Message<impl Replier>) -> bool {
        static NAME_PATTERN: Lazy<Regex> =
            Lazy::new(|| Regex::new("^@?(?:shaken(?:_bot)?|shakey)(?:,.?!:)?").unwrap());

        if msg
            .data
            .split_ascii_whitespace()
            .any(|part| NAME_PATTERN.is_match(part))
        {
            self.last.replace(Instant::now());
            let msg = msg.clone();
            tokio::spawn(async move {
                match Self::generate().await {
                    Ok(data) => msg.reply(responses::Respond { data }),
                    Err(err) => log::error!("cannot generate response: {err}"),
                }
            });
            return true;
        }

        false
    }

    async fn generate() -> anyhow::Result<String> {
        #[derive(serde::Serialize)]
        struct Query {
            min: usize,
            max: usize,
        }

        #[derive(serde::Deserialize)]
        struct Response {
            data: String,
        }

        let resp: Response = reqwest::Client::new()
            .get("http://localhost:10000/generate") // TODO get this from the components
            .json(&Query { min: 5, max: 50 })
            .send()
            .await?
            .json()
            .await?;

        Ok(resp.data)
    }
}
