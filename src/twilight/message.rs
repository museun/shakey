use anyhow::Context;
use std::{collections::HashMap, future::Future, sync::Arc};
use time::OffsetDateTime;
use tokio::sync::{mpsc::UnboundedReceiver, Mutex};
use tokio_stream::StreamExt;
use twilight_gateway::Shard;
use twilight_http::Client;
use twilight_model::{
    channel::message::MessageType,
    gateway::Intents,
    id::{
        marker::{ChannelMarker, MessageMarker},
        Id,
    },
};

use crate::{env::EnvVar, global::GlobalItem, handler::SharedCallable, Reply, Response, Templates};

#[derive(Clone)]
pub struct Message {
    pub inner: Arc<twilight_model::channel::Message>,
    pub source: Arc<str>,
    pub timestamp: OffsetDateTime,
}

impl Message {
    pub(super) fn new(
        inner: twilight_model::channel::Message,
        source: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            inner: Arc::new(inner),
            source: source.into(),
            timestamp: time::OffsetDateTime::now_utc(),
        }
    }
}

impl std::ops::Deref for Message {
    type Target = twilight_model::channel::Message;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
