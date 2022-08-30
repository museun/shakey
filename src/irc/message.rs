use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::{handler::Reply, Response};

use super::lower::Command;

pub trait Replier: Send + Sync + Sized + 'static {
    fn say(item: impl Serialize + Response + 'static) -> Reply<Self>;
    fn reply(item: impl Serialize + Response + 'static) -> Reply<Self>;
    fn problem(item: impl Serialize + Response + 'static) -> Reply<Self>;
}

impl Replier for Box<dyn Response> {
    fn say(item: impl Serialize + Response + 'static) -> Reply<Self> {
        Reply::Say(Box::new(item) as _)
    }

    fn reply(item: impl Serialize + Response + 'static) -> Reply<Self> {
        Reply::Reply(Box::new(item) as _)
    }

    fn problem(item: impl Serialize + Response + 'static) -> Reply<Self> {
        Reply::Problem(Box::new(item) as _)
    }
}

fn erase(item: impl Serialize + Response + 'static) -> Box<[u8]> {
    let d = serde_yaml::to_string(&item).expect("valid yaml");
    let d = Vec::from(d);
    d.into()
}

impl Replier for Box<[u8]> {
    fn say(item: impl Serialize + Response + 'static) -> Reply<Self> {
        Reply::Say(erase(item))
    }

    fn reply(item: impl Serialize + Response + 'static) -> Reply<Self> {
        Reply::Reply(erase(item))
    }

    fn problem(item: impl Serialize + Response + 'static) -> Reply<Self> {
        Reply::Problem(erase(item))
    }
}

pub struct Message<R>
where
    R: Replier,
{
    pub tags: Option<String>,
    pub sender: String,
    pub target: String,
    pub data: String,
    pub timestamp: time::OffsetDateTime,
    pub(crate) reply: UnboundedSender<Reply<R>>,
}

impl<R> std::fmt::Debug for Message<R>
where
    R: Replier,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Message")
            .field("tags", &self.tags)
            .field("sender", &self.sender)
            .field("target", &self.target)
            .field("data", &self.data)
            .finish()
    }
}

impl<R> Clone for Message<R>
where
    R: Replier + 'static,
{
    fn clone(&self) -> Self {
        Self {
            tags: self.tags.clone(),
            sender: self.sender.clone(),
            target: self.target.clone(),
            data: self.data.clone(),
            timestamp: self.timestamp,
            reply: self.reply.clone(),
        }
    }
}

impl<R> Message<R>
where
    R: Replier,
{
    pub fn say(&self, item: impl Serialize + Response + 'static) {
        let item = R::say(item);
        let _ = self.reply.send(item);
    }

    pub fn reply(&self, item: impl Serialize + Response + 'static) {
        let item = R::reply(item);
        let _ = self.reply.send(item);
    }

    pub fn problem(&self, item: impl Serialize + Response + 'static) {
        let item = R::problem(item);
        let _ = self.reply.send(item);
    }

    pub(crate) fn new(command: Command<'_>, reply: UnboundedSender<Reply<R>>) -> Message<R> {
        assert!(matches!(command, Command::Privmsg { .. }));

        if let Command::Privmsg {
            tags,
            sender,
            target,
            data,
        } = command
        {
            return Self {
                tags: tags.map(|s| s.to_string()),
                sender: sender.to_string(),
                target: target.to_string(),
                data: data.to_string(),
                timestamp: time::OffsetDateTime::now_utc(),
                reply,
            };
        }

        unreachable!()
    }
}
