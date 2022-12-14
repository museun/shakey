use std::{collections::VecDeque, path::PathBuf, sync::Arc};

use anyhow::Context;
use rspotify::{
    model::{FullTrack, PlayableItem, TrackId},
    prelude::{Id, OAuthClient},
    AuthCodeSpotify, Credentials, OAuth,
};
use tokio::sync::Mutex;

use crate::ext::IterExt;

struct Queue<T> {
    max: usize,
    queue: VecDeque<T>,
}

impl<T> Queue<T> {
    pub fn with_size(max: usize) -> Self {
        Self {
            max,
            queue: VecDeque::with_capacity(max),
        }
    }

    pub fn push(&mut self, item: T) {
        while self.queue.len() >= self.max {
            self.queue.pop_front();
        }
        self.queue.push_back(item);
    }

    pub fn previous(&self) -> Option<&T> {
        self.queue.iter().rev().nth(1).or_else(|| self.last())
    }

    pub fn last(&self) -> Option<&T> {
        self.queue.back()
    }
}

#[derive(Clone)]
pub struct Song {
    pub id: TrackId,
    pub artist: String,
    pub title: String,
    pub link: String,
}

#[derive(Clone)]
pub struct SpotifyClient {
    client: Arc<AuthCodeSpotify>,
    seen: Arc<Mutex<Queue<Song>>>,
}

impl SpotifyClient {
    pub async fn new(client_id: &str, client_secret: &str) -> anyhow::Result<Self> {
        let credentials = Credentials::new(client_id, client_secret);

        let oauth = OAuth::from_env(rspotify::scopes!(
            "user-read-playback-state",
            "user-read-currently-playing"
        ))
        .with_context(|| "cannot get rspotify oauth pref")?;

        let config = rspotify::Config {
            token_cached: true,
            token_refreshing: true,
            // TODO use the configuration for this
            cache_path: PathBuf::from(std::env::var("RSPOTIFY_TOKEN_CACHE_FILE")?),
            ..rspotify::Config::default()
        };

        let mut auth = AuthCodeSpotify::with_config(credentials, oauth, config);
        let url = auth.get_authorize_url(false)?;
        auth.prompt_for_token(&url).await?;

        let this = Self {
            client: Arc::new(auth),
            seen: Arc::new(Mutex::new(Queue::with_size(10))),
        };

        let (client, seen) = (this.client.clone(), this.seen.clone());
        tokio::spawn(Self::watch_songs(client, seen));

        Ok(this)
    }

    pub async fn previous(&self) -> Option<Song> {
        self.seen.lock().await.previous().cloned()
    }

    pub async fn current(&self) -> Option<Song> {
        self.seen.lock().await.last().cloned()
    }

    async fn watch_songs(client: Arc<AuthCodeSpotify>, seen: Arc<Mutex<Queue<Song>>>) {
        let mut init = false;

        loop {
            if init {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            init = true;

            let track = match Self::look_up_current(&*client).await {
                Ok(Some(track)) => track,
                _ => continue,
            };

            let id = match &track.id {
                Some(id) => id.clone(),
                None => continue,
            };

            let make_song = move |track: FullTrack, id: TrackId| Song {
                artist: track.artists.into_iter().map(|c| c.name).join_with(", "),
                title: track.name,
                link: id.url(),
                id,
            };

            let mut queue = seen.lock().await;
            match queue.last() {
                Some(last) if last.id != id => queue.push(make_song(track, id)),
                None => queue.push(make_song(track, id)),
                _ => continue,
            }
        }
    }

    async fn look_up_current(client: &AuthCodeSpotify) -> anyhow::Result<Option<FullTrack>> {
        let current = client
            .current_playing(None, Option::<Option<_>>::None)
            .await?;

        let current = match current {
            Some(current) => current,
            None => return Ok(None),
        };

        if !current.is_playing {
            return Ok(None);
        }

        if let Some(PlayableItem::Track(track)) = current.item {
            return Ok(Some(track));
        }

        Ok(None)
    }
}
