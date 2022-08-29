use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;

use crate::{handler::Reply, util::ArcCow, Response};

use super::lower::Command;

#[derive(Clone)]
pub struct Message<'a> {
    pub tags: Option<ArcCow<'a>>,
    pub sender: ArcCow<'a>,
    pub target: ArcCow<'a>,
    pub data: ArcCow<'a>,
    reply: UnboundedSender<Reply<Box<dyn Response>>>,
}

impl<'a> Message<'a> {
    pub fn as_owned(&self) -> Message<'static> {
        let this = self.clone();
        Message {
            tags: this.tags.map(ArcCow::into_owned),
            sender: this.sender.into_owned(),
            target: this.target.into_owned(),
            data: this.data.into_owned(),
            reply: this.reply.clone(),
        }
    }

    pub fn say(&self, item: impl Serialize + Response + 'static) {
        let _ = self.reply.send(Reply::Say(Box::new(item)));
    }

    pub fn reply(&self, item: impl Serialize + Response + 'static) {
        let _ = self.reply.send(Reply::Reply(Box::new(item)));
    }

    pub fn problem(&self, item: impl Serialize + Response + 'static) {
        let _ = self.reply.send(Reply::Problem(Box::new(item)));
    }

    pub(crate) fn new(
        command: &'a Command<'_>,
        reply: UnboundedSender<Reply<Box<dyn Response>>>,
    ) -> Message<'a> {
        assert!(matches!(command, Command::Privmsg { .. }));

        if let Command::Privmsg {
            tags,
            sender,
            target,
            data,
        } = command
        {
            return Self {
                tags: tags.map(ArcCow::Borrowed),
                sender: ArcCow::Borrowed(sender),
                target: ArcCow::Borrowed(target),
                data: ArcCow::Borrowed(data),
                reply,
            };
        }

        unreachable!()
    }
}
