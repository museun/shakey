use crate::{
    handler::{Bindable, Components},
    spotify::{Song, SpotifyClient},
    Arguments, Bind, Message, Outcome, Replier,
};

crate::make_response! {
    module: "spotify"

    struct CurrentSong {
        artist: String,
        title: String,
        link: String
    } is "current_song"

    struct PreviousSong {
        artist: String,
        title: String,
        link: String
    } is "previous_song"


    struct Demo {
        msg: String,
    } is "demo"

    struct NotPlaying {
    } is "not_playing"
}

pub struct Spotify {
    client: SpotifyClient,
}

#[async_trait::async_trait]
impl<R: Replier> Bindable<R> for Spotify {
    type Responses = responses::Responses;
    async fn bind(components: &Components) -> anyhow::Result<Bind<Self, R>> {
        let this = Self {
            client: components.get(),
        };
        Bind::create(this)?
            .bind(Self::current_song)?
            .bind(Self::previous_song)
    }
}

impl Spotify {
    fn current_song(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        let msg = msg.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            let Song {
                artist,
                title,
                link,
                ..
            } = match client.current().await {
                Some(song) => song,
                None => return msg.say(responses::NotPlaying {}),
            };

            let item = responses::CurrentSong {
                artist,
                title,
                link,
            };
            msg.say(item)
        })
    }

    fn previous_song(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
        let msg = msg.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            let Song {
                artist,
                title,
                link,
                ..
            } = match client.previous().await {
                Some(song) => song,
                None => return msg.say(responses::NotPlaying {}),
            };

            let item = responses::CurrentSong {
                artist,
                title,
                link,
            };
            msg.say(item)
        })
    }
}
