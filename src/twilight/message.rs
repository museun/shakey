use std::sync::Arc;
use time::OffsetDateTime;

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
