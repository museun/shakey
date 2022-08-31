use std::sync::Arc;

use crate::{handler::Components, helix::EmoteMap, irc::Message, Bind, Replier};

crate::make_response! {
    module: "another_viewer"

    struct Respond {
        data: String
    } is "respond"
}

pub struct AnotherViewer {
    emote_map: Arc<EmoteMap>,
}

impl AnotherViewer {
    pub async fn bind<R: Replier>(components: Components) -> anyhow::Result<Bind<Self, R>> {
        Bind::create::<responses::Responses>(Self {
            emote_map: components.emote_map,
        })?
        .listen(Self::listen)
    }

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
