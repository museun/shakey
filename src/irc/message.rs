use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::{handler::Reply, Replier, Response};

use super::lower::Command;

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

    pub fn is_from_broadcaster(&self) -> bool {
        self.badges_iter()
            .any(|(k, v)| k == "broadcaster" && v == "1")
    }

    pub fn is_from_moderator(&self) -> bool {
        self.badges_iter()
            .any(|(k, v)| k == "moderator" && v == "1")
    }

    pub fn is_from_elevated(&self) -> bool {
        self.is_from_broadcaster() || self.is_from_moderator()
    }

    fn badges_iter(&self) -> impl Iterator<Item = (&'_ str, &'_ str)> + '_ {
        self.tags.iter().flat_map(|tags| {
            tags.split(';')
                .flat_map(|val| val.split_once('='))
                .filter_map(|(k, v)| (k == "badges").then_some(v))
                .flat_map(|v| v.split(','))
                .flat_map(|v| v.split_once('/'))
        })
    }

    pub(crate) fn new(command: Command<'_>, reply: UnboundedSender<Reply<R>>) -> Self {
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
