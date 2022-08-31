// TODO this should also work with tokio::task::JoinHandle<impl Outcome>

use tokio::task::JoinHandle;

pub trait Outcome: Sized {
    fn is_error(&self) -> bool {
        false
    }

    fn into_error(self) -> Option<String> {
        None
    }

    fn into_task(self) -> Option<JoinHandle<anyhow::Result<()>>> {
        None
    }
}

impl Outcome for () {}

impl Outcome for anyhow::Result<()> {
    fn is_error(&self) -> bool {
        matches!(self, Self::Err { .. })
    }

    fn into_error(self) -> Option<String> {
        match self {
            Ok(..) => None,
            Err(resp) => Some(resp.to_string()),
        }
    }
}

impl Outcome for JoinHandle<()> {}

impl Outcome for JoinHandle<anyhow::Result<()>> {
    fn into_task(self) -> Option<Self> {
        Some(self)
    }
}

impl<T> From<()> for MaybeTask<T> {
    fn from(_: ()) -> Self {
        Self::Nope
    }
}

impl<T> From<JoinHandle<T>> for MaybeTask<T> {
    fn from(handle: JoinHandle<T>) -> Self {
        Self::Task(handle)
    }
}

pub enum MaybeTask<T> {
    Task(JoinHandle<T>),
    Nope,
}

impl Outcome for MaybeTask<anyhow::Result<()>> {
    fn into_task(self) -> Option<JoinHandle<anyhow::Result<()>>> {
        match self {
            MaybeTask::Task(task) => Some(task),
            MaybeTask::Nope => None,
        }
    }
}
