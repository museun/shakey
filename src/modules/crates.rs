use std::borrow::Cow;

use crate::{
    ext::DurationSince, handler::Components, irc::Message, Arguments, Bind, Outcome, Replier,
};
use serde::{Deserialize, Deserializer};
use time::{format_description::FormatItem, macros::format_description, OffsetDateTime};

crate::make_response! {
    module: "crates"

    struct Crate {
        name: String,
        version: String,
        description: Cow<'static, str>,
        docs: String,
        repo: Cow<'static, str>,
        updated: String,
    } is "crate"

    struct CrateBestMatch {
        name: String,
        version: String,
        description: Cow<'static, str>,
        docs: String,
        repo: Cow<'static, str>,
        updated: String,
    } is "crate_best_match"

    struct NotFound {
        query: String
    } is "not_found"
}

#[derive(serde::Deserialize, Clone, Debug)]
struct Crate {
    name: String,
    max_version: String,
    description: Option<String>,
    documentation: Option<String>,
    repository: Option<String>,
    exact_match: bool,
    #[serde(deserialize_with = "crates_utc_date_time")]
    updated_at: OffsetDateTime,
}

fn crates_utc_date_time<'de, D>(deser: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error as _;
    const FORMAT: &[FormatItem<'static>] = format_description!(
        "[year]-[month]-[day]T\
            [hour]:[minute]:[second]\
            .[subsecond digits:6]\
            [offset_hour sign:mandatory]:[offset_minute]"
    );
    let s = <Cow<'_, str>>::deserialize(deser)?;
    OffsetDateTime::parse(&s, &FORMAT).map_err(D::Error::custom)
}

pub struct Crates;

impl Crates {
    pub async fn bind<R: Replier>(_: Components) -> anyhow::Result<Bind<Self, R>> {
        Bind::create::<responses::Responses>(Self)?.bind(Self::lookup_crate)
    }

    fn lookup_crate(&mut self, msg: &Message<impl Replier>, mut args: Arguments) -> impl Outcome {
        let query = args.take("crate");
        let msg = msg.clone();
        tokio::spawn(Self::lookup(msg, query))
    }

    async fn lookup(msg: Message<impl Replier>, query: String) -> anyhow::Result<()> {
        #[derive(serde::Deserialize)]
        struct Resp {
            crates: Vec<Crate>,
        }

        let mut resp: Resp = reqwest::Client::new()
            .get("https://crates.io/api/v1/crates")
            .header("User-Agent", crate::USER_AGENT)
            .query(&&[("page", "1"), ("per_page", "1"), ("q", &query)])
            .send()
            .await?
            .json()
            .await?;

        if resp.crates.is_empty() {
            msg.say(responses::NotFound { query });
            return Ok(());
        }

        let mut crate_ = resp.crates.remove(0);

        let description = crate_
            .description
            .take()
            .map(|s| s.replace('\n', " ").trim().to_string())
            .map(Cow::from)
            .unwrap_or_else(|| Cow::from("no description"));

        let docs = crate_
            .documentation
            .take()
            .unwrap_or_else(|| format!("https://docs.rs/{}", crate_.name));

        let repo = crate_
            .repository
            .take()
            .map(Cow::from)
            .unwrap_or_else(|| Cow::from("no repository"));

        let updated = crate_.updated_at.duration_since_now_utc_human();

        if crate_.exact_match {
            msg.say(responses::Crate {
                name: crate_.name,
                version: crate_.max_version,
                description,
                docs,
                repo,
                updated,
            })
        } else {
            msg.say(responses::CrateBestMatch {
                name: crate_.name,
                version: crate_.max_version,
                description,
                docs,
                repo,
                updated,
            })
        }

        Ok(())
    }
}
