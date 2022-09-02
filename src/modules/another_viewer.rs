use std::sync::Arc;

use crate::{
    handler::{Bindable, Components},
    helix::EmoteMap,
    Bind, Message, Replier,
};

crate::make_response! {
    module: "another_viewer"

    struct Respond {
        data: String
    } is "respond"
}

pub struct AnotherViewer {
    emote_map: Arc<EmoteMap>,
}

#[async_trait::async_trait]
impl<R: Replier> Bindable<R> for AnotherViewer {
    type Responses = responses::Responses;

    async fn bind(components: &Components) -> anyhow::Result<Bind<Self, R>> {
        let this = Self {
            emote_map: components.get(),
        };
        Bind::create(this)?.listen(Self::listen)
    }
}

impl AnotherViewer {
    fn listen(&mut self, msg: &Message<impl Replier>) {
        if self.try_kappa(msg) {
            return;
        }
    }

    fn try_kappa(&self, msg: &Message<impl Replier>) -> bool {
        let mut parts = msg
            .data
            .split_ascii_whitespace()
            .filter(|part| self.emote_map.has(part))
            .collect::<Vec<_>>();

        fastrand::shuffle(&mut parts);

        for part in parts {
            if self.emote_map.has(part) {
                msg.say(responses::Respond {
                    data: part.to_string(),
                });
                return true;
            }
        }
        false
    }
}
