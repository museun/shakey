#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use std::{collections::HashMap, sync::Arc};

use once_cell::sync::OnceCell;
use parking_lot::{Mutex, MutexGuard};
use serde::Serialize;
use time::OffsetDateTime;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

use crate::{
    ext::ArcExt,
    responses::{RequiresAdmin, RequiresPermission},
    Replier, Reply, Response,
};

pub fn add_message(msg: MessageKind) -> Uuid {
    get_message_static()
        .insert(msg)
        .unwrap_or_else(|_| panic!("uuid should be unique"))
}

#[derive(Clone)]
pub enum MessageKind {
    Twitch(crate::irc::Message),
    Discord(crate::twilight::Message),
}

pub trait NarrowMessageKind: Sized {
    fn is_twitch(&self) -> bool;
    fn is_discord(&self) -> bool;

    fn into_twitch(self) -> Option<crate::irc::Message>;
    fn into_discord(self) -> Option<crate::twilight::Message>;
}

impl NarrowMessageKind for Arc<MessageKind> {
    fn into_twitch(self) -> Option<crate::irc::Message> {
        match self.unwrap_or_clone() {
            MessageKind::Twitch(twitch) => Some(twitch),
            _ => None,
        }
    }

    fn into_discord(self) -> Option<crate::twilight::Message> {
        match self.unwrap_or_clone() {
            MessageKind::Discord(discord) => Some(discord),
            _ => None,
        }
    }

    fn is_twitch(&self) -> bool {
        matches!(&**self, MessageKind::Twitch { .. })
    }

    fn is_discord(&self) -> bool {
        matches!(&**self, MessageKind::Discord { .. })
    }
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
enum SenderPriv {
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

    priv_: SenderPriv,
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
    fn store_message(kind: MessageKind) -> Uuid {
        add_message(kind)
    }

    pub(crate) fn twitch(msg: crate::irc::Message, reply: UnboundedSender<Reply<R>>) -> Self {
        let is_broadcaster =
            |k: &str, v: &str| (k == "broadcaster" && v == "1").then_some(SenderPriv::Admin);
        let is_moderator =
            |k: &str, v: &str| (k == "moderator" && v == "1").then_some(SenderPriv::Moderator);

        let priv_ = msg
            .badges_iter()
            .find_map(|(k, v)| is_broadcaster(k, v).or_else(|| is_moderator(k, v)))
            .unwrap_or_default();

        Self {
            id: Self::store_message(MessageKind::Twitch(msg.clone())),
            timestamp: msg.timestamp,
            sender: msg.sender,
            target: msg.target,
            data: msg.data,
            priv_,
            reply,
        }
    }

    pub(crate) fn discord(msg: crate::twilight::Message, reply: UnboundedSender<Reply<R>>) -> Self {
        Self {
            id: Self::store_message(MessageKind::Discord(msg.clone())),
            timestamp: msg.timestamp,
            sender: msg.inner.author.name.clone().into(),
            target: msg.source,
            data: msg.inner.content.clone().into(),
            priv_: SenderPriv::default(),
            reply,
        }
    }
}

impl<R: Replier> Message<R> {
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
}

impl<R: Replier> Message<R> {
    pub const fn id(&self) -> Uuid {
        self.id
    }

    pub const fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
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
}

impl<R: Replier> Message<R> {
    pub const fn is_from_broadcaster(&self) -> bool {
        matches!(self.priv_, SenderPriv::Admin)
    }

    pub const fn is_from_moderator(&self) -> bool {
        matches!(self.priv_, SenderPriv::Moderator)
    }

    pub const fn is_from_elevated(&self) -> bool {
        !(self.is_from_broadcaster() || self.is_from_moderator())
    }

    pub fn require_broadcaster(&self) -> bool {
        if !self.is_from_broadcaster() {
            self.problem(RequiresAdmin {});
            return false;
        }
        true
    }

    pub fn requires_permission(&self) -> bool {
        if !self.is_from_elevated() {
            self.problem(RequiresPermission {});
            return false;
        }
        true
    }

    pub fn is_discord(&self) -> bool {
        lookup_message(self.id())
            .filter(NarrowMessageKind::is_discord)
            .is_some()
    }

    pub fn is_twitch(&self) -> bool {
        lookup_message(self.id())
            .filter(NarrowMessageKind::is_twitch)
            .is_some()
    }

    pub fn as_discord(&self) -> Option<crate::twilight::Message> {
        lookup_message(self.id()).and_then(NarrowMessageKind::into_discord)
    }

    pub fn as_twitch(&self) -> Option<crate::irc::Message> {
        lookup_message(self.id()).and_then(NarrowMessageKind::into_twitch)
    }
}
