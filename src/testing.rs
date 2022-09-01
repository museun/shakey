use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::{
    global::GLOBAL_TEMPLATES,
    handler::{Bind, Reply, Response},
    irc::Message,
    Callable,
};

pub struct MockBinding<T: Send + Sync + 'static> {
    inner: Bind<T, Box<[u8]>>,
    send: UnboundedSender<Reply<Box<[u8]>>>,
    recv: UnboundedReceiver<Reply<Box<[u8]>>>,
}

impl<T: Send + Sync + 'static> MockBinding<T> {
    pub fn send(&mut self, data: &str) {
        let message = Message {
            tags: None,
            sender: "test_user".into(),
            target: "test_channel".into(),
            data: data.into(),
            timestamp: time::OffsetDateTime::now_utc(),
            reply: self.send.clone(),
        };
        self.inner.call_func(&message);
        self.recv.close()
    }

    pub async fn read_all_untyped(&mut self) -> Vec<Reply<String>> {
        let mut out = vec![];
        while let Some(recv) = self.recv.recv().await {
            let data = recv
                .map(Vec::from)
                .map(String::from_utf8)
                .map(Result::unwrap);
            out.push(data);
        }
        out
    }

    pub async fn read_untyped(&mut self) -> Reply<String> {
        let data = self.recv.recv().await.expect("response");
        data.map(Vec::from)
            .map(String::from_utf8)
            .map(Result::unwrap)
    }

    pub async fn expect_response<R>(&mut self) -> Reply<R>
    where
        R: Response + for<'de> Deserialize<'de> + 'static,
    {
        let data = self.recv.recv().await.expect("response");
        data.map(|data| serde_yaml::from_slice(&data).expect("valid yaml"))
    }

    pub async fn expect_say<R>(&mut self) -> R
    where
        R: Response + for<'de> Deserialize<'de> + 'static,
        R: std::fmt::Debug,
    {
        match self.expect_response::<R>().await {
            Reply::Say(data) => data,
            Reply::Reply(data) => panic!("expected a say, got a reply: {data:?}"),
            Reply::Problem(data) => panic!("expected a say, got a problem: {data:?}"),
        }
    }

    pub async fn expect_reply<R>(&mut self) -> R
    where
        R: Response + for<'de> Deserialize<'de> + 'static,
        R: std::fmt::Debug,
    {
        match self.expect_response::<R>().await {
            Reply::Reply(data) => data,
            Reply::Say(data) => panic!("expected a reply, got a say: {data:?}"),
            Reply::Problem(data) => panic!("expected a reply, got a problem: {data:?}"),
        }
    }

    pub async fn expect_problem<R>(&mut self) -> R
    where
        R: Response + for<'de> Deserialize<'de> + 'static,
        R: std::fmt::Debug,
    {
        match self.expect_response::<R>().await {
            Reply::Problem(data) => data,
            Reply::Reply(data) => panic!("expected a problem, got a reply: {data:?}"),
            Reply::Say(data) => panic!("expected a problem, got a say: {data:?}"),
        }
    }
}

pub fn mock<T, F>(ctor: F) -> MockBinding<T>
where
    T: Send + Sync + 'static,
    F: Fn() -> anyhow::Result<Bind<T, Box<[u8]>>> + Send + 'static,
{
    initialize_global_state();

    let (tx, rx) = unbounded_channel();
    MockBinding {
        inner: ctor().expect("binding"),
        send: tx,
        recv: rx,
    }
}

fn initialize_global_state() {
    let templates = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", "templates.yaml"));
    let templates = serde_yaml::from_str(templates).map(Arc::new).unwrap();
    GLOBAL_TEMPLATES.initialize(templates);
    crate::bind_system_errors().unwrap();

    let commands = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", "commands.yaml"));
    let commands = serde_yaml::from_str(commands).map(Arc::new).unwrap();
    GLOBAL_TEMPLATES.initialize(commands);
}
