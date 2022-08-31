use serde::Serialize;

use crate::{Reply, Response};

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
