use std::sync::Arc;

use super::raw::Command;

#[derive(Clone)]
pub struct Message {
    pub tags: Option<Arc<str>>,
    pub sender: Arc<str>,
    pub target: Arc<str>,
    pub data: Arc<str>,
    pub timestamp: time::OffsetDateTime,
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Message")
            .field("tags", &self.tags)
            .field("sender", &self.sender)
            .field("target", &self.target)
            .field("data", &self.data)
            .finish()
    }
}

impl Message {
    pub fn badges_iter(&self) -> impl Iterator<Item = (&'_ str, &'_ str)> + '_ {
        self.tags.iter().flat_map(|tags| {
            tags.split(';')
                .flat_map(|val| val.split_once('='))
                .filter_map(|(k, v)| (k == "badges").then_some(v))
                .flat_map(|v| v.split(','))
                .flat_map(|v| v.split_once('/'))
        })
    }

    pub(crate) fn new(command: Command<'_>) -> Self {
        assert!(matches!(command, Command::Privmsg { .. }));

        if let Command::Privmsg {
            tags,
            sender,
            target,
            data,
        } = command
        {
            return Self {
                tags: tags.map(|s| s.into()),
                sender: sender.into(),
                target: target.into(),
                data: data.into(),
                timestamp: time::OffsetDateTime::now_utc(),
            };
        }

        unreachable!()
    }
}
