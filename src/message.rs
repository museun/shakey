#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use std::{collections::HashMap, sync::Arc};

use once_cell::sync::OnceCell;
use parking_lot::{Mutex, MutexGuard};
use serde::Serialize;
use time::OffsetDateTime;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::{ext::ArcExt, Replier, Reply, Response};

pub fn add_message(msg: MessageKind) -> Uuid {
    get_message_static()
        .insert(msg)
        .unwrap_or_else(|_| panic!("uuid should be unique"))
}

#[derive(Clone)]
pub enum MessageKind {
    Twitch(crate::irc::Message),
    Discord(inner::Discord),
}

pub trait NarrowMessageKind: Sized {
    fn as_twitch(self) -> Option<crate::irc::Message>;
    fn as_discord(self) -> Option<inner::Discord>;
}

impl NarrowMessageKind for Arc<MessageKind> {
    fn as_twitch(self) -> Option<crate::irc::Message> {
        match self.unwrap_or_clone() {
            MessageKind::Twitch(twitch) => Some(twitch),
            _ => None,
        }
    }

    fn as_discord(self) -> Option<inner::Discord> {
        match self.unwrap_or_clone() {
            MessageKind::Discord(discord) => Some(discord),
            _ => None,
        }
    }
}

pub mod inner {
    #[derive(Clone)]
    pub struct Twitch {}

    #[derive(Clone)]
    pub struct Discord {}
}

static MESSAGES: OnceCell<Mutex<Messages>> = OnceCell::new();

fn get_message_static() -> MutexGuard<'static, Messages> {
    MESSAGES.get_or_init(|| Mutex::new(Messages::new())).lock()
}

fn lookup_message(id: Uuid) -> Option<Arc<MessageKind>> {
    get_message_static().get(id)
}

struct Messages {
    map: HashMap<Uuid, Arc<MessageKind>>,
}

impl Messages {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn get(&self, id: Uuid) -> Option<Arc<MessageKind>> {
        self.map.get(&id).cloned()
    }

    fn insert(&mut self, item: MessageKind) -> Result<Uuid, MessageKind> {
        use std::collections::hash_map::Entry::*;
        let id = Uuid::new_v4();
        match self.map.entry(id) {
            Occupied(..) => Err(item),
            Vacant(e) => {
                e.insert(Arc::new(item));
                Ok(id)
            }
        }
    }
}

#[derive(Copy, Clone, Default)]
pub(crate) enum SenderPriv {
    Admin,
    Moderator,
    #[default]
    None,
}

pub struct Message<R: Replier> {
    pub(crate) id: Uuid,
    pub(crate) timestamp: OffsetDateTime,
    pub(crate) sender: Arc<str>,
    pub(crate) target: Arc<str>,
    pub(crate) data: Arc<str>,

    pub(crate) priv_: SenderPriv,
    pub(crate) reply: UnboundedSender<Reply<R>>,
}

impl<R: Replier> std::fmt::Debug for Message<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Message")
            .field("id", &self.id)
            .field("sender", &self.sender)
            .field("target", &self.target)
            .field("data", &self.data)
            .finish()
    }
}

impl<R: Replier + 'static> Clone for Message<R> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            timestamp: self.timestamp,
            sender: self.sender.clone(),
            target: self.target.clone(),
            data: self.data.clone(),

            priv_: self.priv_,
            reply: self.reply.clone(),
        }
    }
}

impl<R: Replier> Message<R> {
    pub(crate) fn twitch(msg: crate::irc::Message, reply: UnboundedSender<Reply<R>>) -> Self {
        let is_broadcaster =
            |k: &str, v: &str| (k == "broadcaster" && v == "1").then_some(SenderPriv::Admin);
        let is_moderator =
            |k: &str, v: &str| (k == "moderator" && v == "1").then_some(SenderPriv::Moderator);

        let priv_ = msg
            .badges_iter()
            .find_map(|(k, v)| is_broadcaster(k, v).or_else(|| is_moderator(k, v)))
            .unwrap_or_default();

        let crate::irc::Message {
            sender,
            target,
            data,
            timestamp,
            ..
        } = msg;

        Self {
            id: Uuid::new_v4(),
            timestamp,
            sender,
            target,
            data,
            priv_,
            reply,
        }
    }

    pub(crate) fn discord(msg: crate::twilight::Message, reply: UnboundedSender<Reply<R>>) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: msg.timestamp,
            sender: msg.inner.author.name.into(),
            target: msg.source,
            data: msg.inner.content.into(),
            priv_: SenderPriv::default(),
            reply,
        }
    }

    pub const fn id(&self) -> Uuid {
        self.id
    }

    pub const fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
    }

    pub fn as_discord(&self) -> Option<inner::Discord> {
        lookup_message(self.id()).and_then(NarrowMessageKind::as_discord)
    }

    pub fn as_twitch(&self) -> Option<crate::irc::Message> {
        lookup_message(self.id()).and_then(NarrowMessageKind::as_twitch)
    }

    pub fn sender(&self) -> &str {
        &self.sender
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn data(&self) -> &str {
        &self.data
    }

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

    pub const fn is_from_broadcaster(&self) -> bool {
        matches!(self.priv_, SenderPriv::Admin)
    }

    pub const fn is_from_moderator(&self) -> bool {
        matches!(self.priv_, SenderPriv::Moderator)
    }

    pub const fn is_from_elevated(&self) -> bool {
        !matches!(self.priv_, SenderPriv::None)
    }
}
