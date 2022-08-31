#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use std::sync::Arc;

use shakey::{get_env_var, irc, Commands, Templates};

async fn initialize_templates() -> anyhow::Result<()> {
    let path = get_env_var("SHAKEN_TEMPLATES_PATH")?;
    let templates = Templates::load_from_yaml(&path).await.map(Arc::new)?;
    shakey::global::GLOBAL_TEMPLATES.initialize(templates);
    shakey::bind_system_errors()?;

    tokio::spawn(async move {
        if let Err(err) = Templates::watch_for_updates(path).await {
            log::error!("could not reload templates: {err}");
            std::process::exit(0) // TODO not this
        }
    });

    Ok(())
}

async fn initialize_commands() -> anyhow::Result<()> {
    let path = get_env_var("SHAKEN_COMMANDS_PATH")?;
    let commands = Commands::load_from_yaml(&path).await.map(Arc::new)?;
    shakey::global::GLOBAL_COMMANDS.initialize(commands);

    tokio::spawn(async move {
        if let Err(err) = Commands::watch_for_updates(path).await {
            log::error!("could not reload commands: {err}");
            std::process::exit(0) // TODO not this
        }
    });

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    simple_env_load::load_env_from([".dev.env", ".secrets.env"]);
    alto_logger::init_term_logger()?;

    let helix_client_id = get_env_var("SHAKEN_TWITCH_CLIENT_ID")?;
    let helix_client_secret = get_env_var("SHAKEN_TWITCH_CLIENT_SECRET")?;

    let helix_oauth = shakey::helix::OAuth::create(&helix_client_id, &helix_client_secret).await?;
    let helix_client = shakey::helix::HelixClient::new(helix_oauth);

    let spotify_client_id = get_env_var("SHAKEN_SPOTIFY_CLIENT_ID")?;
    let spotify_client_secret = get_env_var("SHAKEN_SPOTIFY_CLIENT_SECRET")?;
    let spotify_client =
        spotify::SpotifyClient::new(&spotify_client_id, &spotify_client_secret).await?;

    let gist_id = get_env_var("SHAKEN_SETTINGS_GIST_ID")?;
    let gist_id = Arc::<str>::from(&*gist_id);

    let github_oauth_token = get_env_var("SHAKEN_GITHUB_OAUTH_TOKEN")?;
    let oauth = Arc::new(vscode::OAuth {
        token: github_oauth_token,
    });

    loop {
        initialize_commands().await?;
        initialize_templates().await?;

        let builtin = builtin::Builtin::bind().await?.into_callable();
        let twitch = twitch::Twitch::bind(helix_client.clone())
            .await?
            .into_callable();

        let spotify = spotify::Spotify::bind(spotify_client.clone())
            .await?
            .into_callable();

        let crates = crates::Crates::bind().await?.into_callable();

        let vscode = vscode::Vscode::bind(gist_id.clone(), oauth.clone())
            .await?
            .into_callable();

        let help = help::Help::bind().await?.into_callable();

        let user_defined = user_defined::UserDefined::bind().await?.into_callable();

        if let Err(err) = async move {
            shakey::irc::run([
                builtin, //
                twitch,  //
                spotify, //
                crates,  //
                vscode,  //
                help,    //
                user_defined,
            ])
            .await?;
            anyhow::Result::<_, anyhow::Error>::Ok(())
        }
        .await
        {
            log::warn!("disconnected");
            match () {
                _ if err.is::<irc::errors::Connection>() => {}
                _ if err.is::<irc::errors::Eof>() => {}
                _ if err.is::<irc::errors::Timeout>() => {}
                _ => {
                    log::error!("{err}");
                    std::process::exit(1)
                }
            }

            log::warn!("reconnecting in 5 seconds");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }
}

mod twitch {
    use shakey::{
        ext::FormatTime,
        helix::{data::Stream, HelixClient},
        irc::Message,
        Arguments, Bind, Outcome, Replier,
    };
    use time::OffsetDateTime;

    shakey::make_response! {
        module: "twitch"

        struct Viewers {
            name: String,
            viewers: u64
        } is "viewers"

        struct Uptime {
            name: String,
            uptime: String,
        } is "uptime"

        struct NotStreaming {
            channel: String,
        } is "not_streaming"
    }

    pub struct Twitch {
        client: HelixClient,
    }

    impl Twitch {
        pub async fn bind<R: Replier + 'static>(
            client: HelixClient,
        ) -> anyhow::Result<Bind<Self, R>> {
            Bind::create::<responses::Responses>(Self { client })?
                .bind(Self::uptime)?
                .bind(Self::viewers)
        }

        fn uptime(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
            async fn uptime_async(
                client: HelixClient,
                msg: Message<impl Replier>,
                args: Arguments,
            ) -> anyhow::Result<()> {
                let Stream {
                    user_name: name,
                    started_at,
                    ..
                } = match Twitch::get_stream(&client, &msg, &args).await? {
                    Some(stream) => stream,
                    None => return Ok(()),
                };

                let uptime = (OffsetDateTime::now_utc() - started_at).as_readable_time();
                msg.say(responses::Uptime { name, uptime });

                Ok(())
            }

            let msg = msg.clone();
            let client = self.client.clone();
            tokio::spawn(uptime_async(client, msg, args))
        }

        fn viewers(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
            async fn viewers(
                client: HelixClient,
                msg: Message<impl Replier>,
                args: Arguments,
            ) -> anyhow::Result<()> {
                let Stream {
                    user_name: name,
                    viewer_count: viewers,
                    ..
                } = match Twitch::get_stream(&client, &msg, &args).await? {
                    Some(stream) => stream,
                    None => return Ok(()),
                };

                msg.say(responses::Viewers { name, viewers });

                Ok(())
            }

            let msg = msg.clone();
            let client = self.client.clone();
            tokio::spawn(viewers(client, msg, args))
        }

        async fn get_stream(
            client: &HelixClient,
            msg: &Message<impl Replier>,
            args: &Arguments,
        ) -> anyhow::Result<Option<Stream>> {
            let channel = args.get("channel").unwrap_or(&msg.target);
            let channel = channel.strip_prefix('#').unwrap_or(channel);

            if let Some(stream) = client.get_streams([channel]).await?.pop() {
                return Ok(Some(stream));
            }

            msg.problem(responses::NotStreaming {
                channel: channel.to_string(),
            });

            Ok(None)
        }
    }
}

mod builtin {
    use std::{borrow::Cow, time::Duration};

    use fastrand::Rng;
    use fastrand_ext::SliceExt;
    use shakey::{
        data::{FileTypes, Interest, Watch, WatchFile},
        ext::FormatTime,
        irc::Message,
        Arguments, Bind, Outcome, Replier,
    };
    use time::{format_description::FormatItem, macros::format_description, OffsetDateTime};
    use tokio::time::Instant;

    shakey::make_response! {
        module: "builtin"

        struct Ping {
            time: String
        } is "ping"

        struct PingWithToken {
            time: String,
            token: String
        } is "ping_with_token"

        struct Hello {
            greeting: String,
            sender: String
        } is "hello"

        struct Time {
            now: String,
        } is "time"

        struct BotUptime {
            uptime: String,
        } is "bot_uptime"

        struct Version {
            revision: String,
            branch: String,
            build_time: String,
        } is "version"

        struct SayHello {
            greeting: String,
            sender: String
        } is "say_hello"
    }

    #[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
    #[serde(transparent)]
    pub struct Greetings(Vec<String>);

    impl Greetings {
        fn contains(&self, key: &str) -> bool {
            self.0.iter().any(|s| s.eq_ignore_ascii_case(key))
        }

        fn choose(&self, rng: &Rng) -> Cow<'_, str> {
            (!self.0.is_empty())
                .then(|| self.0.choose(rng).map(|s| Cow::from(&**s)))
                .flatten()
                .unwrap_or(Cow::Borrowed("hello"))
        }
    }

    impl Interest for Greetings {
        fn module() -> &'static str {
            "builtin"
        }
        fn file() -> &'static str {
            "greetings.yaml"
        }
    }

    pub struct Builtin {
        uptime: Instant,
        greetings: WatchFile<Greetings>,
    }

    impl Builtin {
        pub async fn bind<R>() -> anyhow::Result<Bind<Self, R>>
        where
            R: Replier + 'static,
        {
            let greetings = Greetings::watch().await?;
            let this = Self {
                greetings,
                uptime: Instant::now(),
            };

            Bind::create::<responses::Responses>(this)?
                .bind(Self::ping)?
                .bind(Self::hello)?
                .bind(Self::time)?
                .bind(Self::bot_uptime)?
                .bind(Self::version)?
                .listen(Self::say_hello)
        }

        fn ping(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
            let now = OffsetDateTime::now_local()?;
            let ms: Duration = (now - msg.timestamp).try_into()?;
            let time = format!("{ms:.1?}");

            match args.get("token").map(ToString::to_string) {
                Some(token) => msg.say(responses::PingWithToken { token, time }),
                None => msg.say(responses::Ping { time }),
            }

            Ok(())
        }

        fn hello(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
            async fn hello(msg: Message<impl Replier>, greetings: WatchFile<Greetings>) {
                let greetings = greetings.get().await;
                let greeting = greetings.choose(&fastrand::Rng::new());
                msg.say(responses::Hello {
                    greeting: greeting.to_string(),
                    sender: msg.sender.to_string(),
                })
            }
            let msg = msg.clone();
            let greetings = self.greetings.clone();
            tokio::spawn(hello(msg, greetings))
        }

        fn time(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
            static FMT: &[FormatItem<'static>] = format_description!("[hour]:[minute]:[second]");
            let now = OffsetDateTime::now_local()?.format(&FMT)?;
            msg.say(responses::Time { now });
            Ok(())
        }

        fn bot_uptime(&mut self, msg: &Message<impl Replier>, args: Arguments) {
            let uptime = self.uptime.elapsed().as_readable_time();
            msg.say(responses::BotUptime { uptime })
        }

        fn version(&mut self, msg: &Message<impl Replier>, args: Arguments) {
            msg.say(responses::Version {
                revision: shakey::GIT_REVISION.to_string(),
                branch: shakey::GIT_BRANCH.to_string(),
                build_time: shakey::BUILD_TIME.to_string(),
            })
        }

        fn say_hello(&mut self, msg: &Message<impl Replier>) -> impl Outcome {
            async fn say_hello(
                msg: Message<impl Replier>,
                greetings: WatchFile<Greetings, { FileTypes::YAML }>,
            ) {
                let data = msg.data.trim_end_matches(['!', '?', '.']);
                let greetings = greetings.get().await;
                if !greetings.contains(data) {
                    return;
                }

                let greeting = greetings.choose(&fastrand::Rng::new());
                msg.say(responses::SayHello {
                    greeting: greeting.to_string(),
                    sender: msg.sender.to_string(),
                })
            }

            let msg = msg.clone();
            let greetings = self.greetings.clone();
            tokio::spawn(say_hello(msg, greetings))
        }
    }
}

mod spotify {
    use std::{collections::VecDeque, path::PathBuf, sync::Arc};

    use anyhow::Context;
    use rspotify::{
        model::{FullTrack, PlayableItem, TrackId},
        prelude::{Id, OAuthClient},
        AuthCodeSpotify, Credentials, OAuth,
    };
    use shakey::{ext::IterExt, irc, Arguments, Bind, Outcome, Replier};
    use tokio::sync::Mutex;

    shakey::make_response! {
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
    struct Song {
        id: TrackId,
        artist: String,
        title: String,
        link: String,
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

        async fn previous(&self) -> Option<Song> {
            self.seen.lock().await.previous().cloned()
        }

        async fn current(&self) -> Option<Song> {
            self.seen.lock().await.last().cloned()
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

    pub struct Spotify {
        client: SpotifyClient,
    }

    impl Spotify {
        pub async fn bind<R: Replier>(client: SpotifyClient) -> anyhow::Result<Bind<Self, R>> {
            Bind::create::<responses::Responses>(Self { client })?
                .bind(Self::current_song)?
                .bind(Self::previous_song)
        }

        fn current_song(
            &mut self,
            msg: &irc::Message<impl Replier>,
            args: Arguments,
        ) -> impl Outcome {
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

        fn previous_song(
            &mut self,
            msg: &irc::Message<impl Replier>,
            args: Arguments,
        ) -> impl Outcome {
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
}

mod crates {
    use std::borrow::Cow;

    use serde::{Deserialize, Deserializer};
    use shakey::{ext::DurationSince, irc::Message, Arguments, Bind, Outcome, Replier};
    use time::{format_description::FormatItem, macros::format_description, OffsetDateTime};

    shakey::make_response! {
        module: "crates"

        struct Crate {
            name: String,
            version: String,
            description: Cow<'static, str>,
            docs: String,
            repo: Cow<'static, str>,
            updated: String,
        } is "crate"

        struct CrateBestMatch {
            name: String,
            version: String,
            description: Cow<'static, str>,
            docs: String,
            repo: Cow<'static, str>,
            updated: String,
        } is "crate_best_match"

        struct NotFound {
            query: String
        } is "not_found"
    }

    #[derive(serde::Deserialize, Clone, Debug)]
    struct Crate {
        name: String,
        max_version: String,
        description: Option<String>,
        documentation: Option<String>,
        repository: Option<String>,
        exact_match: bool,
        #[serde(deserialize_with = "crates_utc_date_time")]
        updated_at: OffsetDateTime,
    }

    fn crates_utc_date_time<'de, D>(deser: D) -> Result<OffsetDateTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error as _;
        const FORMAT: &[FormatItem<'static>] = format_description!(
            "[year]-[month]-[day]T\
            [hour]:[minute]:[second]\
            .[subsecond digits:6]\
            [offset_hour sign:mandatory]:[offset_minute]"
        );
        let s = <Cow<'_, str>>::deserialize(deser)?;
        OffsetDateTime::parse(&s, &FORMAT).map_err(D::Error::custom)
    }

    pub struct Crates;

    impl Crates {
        pub async fn bind<R: Replier>() -> anyhow::Result<Bind<Self, R>> {
            Bind::create::<responses::Responses>(Self)?.bind(Self::lookup_crate)
        }

        fn lookup_crate(
            &mut self,
            msg: &Message<impl Replier>,
            mut args: Arguments,
        ) -> impl Outcome {
            let query = args.take("crate");
            let msg = msg.clone();
            tokio::spawn(Self::lookup(msg, query))
        }

        async fn lookup(msg: Message<impl Replier>, query: String) -> anyhow::Result<()> {
            #[derive(serde::Deserialize)]
            struct Resp {
                crates: Vec<Crate>,
            }

            let mut resp: Resp = reqwest::Client::new()
                .get("https://crates.io/api/v1/crates")
                .header("User-Agent", shakey::USER_AGENT)
                .query(&&[("page", "1"), ("per_page", "1"), ("q", &query)])
                .send()
                .await?
                .json()
                .await?;

            if resp.crates.is_empty() {
                msg.say(responses::NotFound { query });
                return Ok(());
            }

            let mut crate_ = resp.crates.remove(0);

            let description = crate_
                .description
                .take()
                .map(|s| s.replace('\n', " ").trim().to_string())
                .map(Cow::from)
                .unwrap_or_else(|| Cow::from("no description"));

            let docs = crate_
                .documentation
                .take()
                .unwrap_or_else(|| format!("https://docs.rs/{}", crate_.name));

            let repo = crate_
                .repository
                .take()
                .map(Cow::from)
                .unwrap_or_else(|| Cow::from("no repository"));

            let updated = crate_.updated_at.duration_since_now_utc_human();

            if crate_.exact_match {
                msg.say(responses::Crate {
                    name: crate_.name,
                    version: crate_.max_version,
                    description,
                    docs,
                    repo,
                    updated,
                })
            } else {
                msg.say(responses::CrateBestMatch {
                    name: crate_.name,
                    version: crate_.max_version,
                    description,
                    docs,
                    repo,
                    updated,
                })
            }

            Ok(())
        }
    }
}

mod vscode {
    use std::{collections::HashMap, sync::Arc};

    use anyhow::Context;
    use shakey::{irc::Message, Arguments, Bind, Outcome, Replier};

    shakey::make_response! {
        module: "vscode"

        struct Theme {
            theme_url: String,
            variant: String,
        } is "theme"

        struct Fonts {
            editor: String,
            terminal: String,
        } is "fonts"
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct FontsAndTheme {
        editor_font: String,
        terminal_font: String,
        theme_url: String,
        theme_variant: String,
    }

    pub struct OAuth {
        pub token: String,
    }

    pub struct Vscode {
        gist_id: Arc<str>,
        oauth: Arc<OAuth>,
    }

    impl Vscode {
        pub async fn bind<R: Replier>(
            gist_id: Arc<str>,
            oauth: Arc<OAuth>,
        ) -> anyhow::Result<Bind<Self, R>> {
            Bind::create::<responses::Responses>(Self { gist_id, oauth })?
                .bind(Self::theme)?
                .bind(Self::fonts)
        }

        fn theme(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
            async fn theme(
                msg: Message<impl Replier>,
                gist_id: Arc<str>,
                oauth: Arc<OAuth>,
            ) -> anyhow::Result<()> {
                let FontsAndTheme {
                    theme_url,
                    theme_variant,
                    ..
                } = Vscode::get_current_settings(gist_id, oauth).await?;

                msg.say(responses::Theme {
                    theme_url,
                    variant: theme_variant,
                });

                Ok(())
            }

            let msg = msg.clone();
            let (gist_id, oauth) = (self.gist_id.clone(), self.oauth.clone());
            tokio::spawn(theme(msg, gist_id, oauth))
        }

        fn fonts(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
            async fn fonts(
                msg: Message<impl Replier>,
                gist_id: Arc<str>,
                oauth: Arc<OAuth>,
            ) -> anyhow::Result<()> {
                let FontsAndTheme {
                    editor_font,
                    terminal_font,
                    ..
                } = Vscode::get_current_settings(gist_id, oauth).await?;

                msg.say(responses::Fonts {
                    editor: editor_font,
                    terminal: terminal_font,
                });

                Ok(())
            }

            let msg = msg.clone();
            let (gist_id, oauth) = (self.gist_id.clone(), self.oauth.clone());
            tokio::spawn(fonts(msg, gist_id, oauth))
        }

        async fn get_current_settings(
            gist_id: Arc<str>,
            oauth: Arc<OAuth>,
        ) -> anyhow::Result<FontsAndTheme> {
            #[derive(Debug, ::serde::Deserialize)]
            struct File {
                content: String,
                raw_url: String,
            }

            async fn get_gist_files(
                id: &str,
                OAuth { token }: &OAuth,
            ) -> anyhow::Result<HashMap<String, File>> {
                #[derive(Debug, ::serde::Deserialize)]
                struct Response {
                    files: HashMap<String, File>,
                }

                let resp: Response = [
                    ("Accept", "application/vnd.github+json"),
                    ("Authorization", &format!("token {token}")),
                    ("User-Agent", shakey::USER_AGENT),
                ]
                .into_iter()
                .fold(
                    reqwest::Client::new().get(format!("https://api.github.com/gists/{id}")),
                    |req, (k, v)| req.header(k, v),
                )
                .send()
                .await?
                .json()
                .await?;

                Ok(resp.files)
            }

            let files = get_gist_files(&gist_id, &oauth).await?;
            let file = files
                .get("vscode settings.json") // TODO don't hardcode this
                .with_context(|| "cannot find settings")?;
            serde_json::from_str(&file.content).map_err(Into::into)
        }
    }
}

mod help {
    use shakey::{ext::IterExt, irc::Message, Arguments, Bind, Replier};

    #[derive(
        Default,
        Clone,
        Debug,
        PartialEq,
        PartialOrd,
        Eq,
        Ord,
        Hash,
        serde::Serialize,
        serde::Deserialize,
    )]
    pub struct Maybe<T>(Option<T>);

    impl<T> std::fmt::Display for Maybe<T>
    where
        T: std::fmt::Display,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            if let Self(Some(s)) = self {
                write!(f, "{s} ")?;
            }
            Ok(())
        }
    }

    shakey::make_response! {
        module: "help"

        struct ListCommands {
            commands: String
        } is "list_commands"

        struct SpecificCommandNoAlias {
            command: String,
            usage: super::Maybe<String>,
            description: String,
        } is "specific_command_no_alias"

        struct SpecificCommand {
            command: String,
            usage: super::Maybe<String>,
            description: String,
            aliases: String,
        } is "specific_command"


        struct UnknownCommand {
            command: String
        } is "unknown_command"
    }

    pub struct Help;

    impl Help {
        pub async fn bind<R: Replier>() -> anyhow::Result<Bind<Self, R>> {
            Bind::create::<responses::Responses>(Self)?.bind(Self::help)
        }

        fn help(&mut self, msg: &Message<impl Replier>, args: Arguments) {
            const MAX_PER_LINE: usize = 10;

            let commands = shakey::global::commands();
            match args.get("command") {
                Some(cmd) => match commands.find_by_name(cmd) {
                    Some(cmd) => {
                        let command = cmd.command.clone();
                        let usage =
                            Maybe((!cmd.args.usage.is_empty()).then(|| cmd.args.usage.to_string()));
                        let description = cmd.description.clone();
                        if cmd.aliases.is_empty() {
                            msg.say(responses::SpecificCommandNoAlias {
                                command,
                                usage,
                                description,
                            })
                        } else {
                            let aliases = cmd.aliases.iter().join_with(" ");
                            msg.say(responses::SpecificCommand {
                                command,
                                usage,
                                description,
                                aliases,
                            })
                        }
                    }
                    None => msg.say(responses::UnknownCommand {
                        command: cmd.to_string(),
                    }),
                },
                None => msg.say(responses::ListCommands {
                    commands: commands.command_names().join_multiline_max(MAX_PER_LINE),
                }),
            }
        }
    }
}

mod user_defined {
    use std::collections::HashMap;

    use shakey::{
        data::Interest, ext::IterExt, irc::Message, responses::RequiresPermission, Arguments, Bind,
        Outcome, Replier,
    };

    shakey::make_response! {
        module: "user_defined"

        struct Command {
            body: String
        } is "command"

        struct Updated {
            command: String,
            body: String,
        } is "updated"

        struct Added {
            command: String,
            body: String,
        } is "added"

        struct Removed {
            command: String,
            body: String,
        } is "removed"

        struct Commands {
            commands: String,
        } is "commands"

        struct CommandExists {
            command: String
        } is "command_exists"

        struct CommandNotFound {
            command: String
        } is "command_not_found"

        struct InvalidSyntax {
            error: &'static str
        } is "invalid_syntax"
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct Command {
        command: String,
        body: String,
        uses: usize,
    }

    #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
    struct Commands {
        #[serde(flatten)]
        map: HashMap<String, Command>,
    }

    impl Interest for Commands {
        fn module() -> &'static str {
            "user_defined"
        }

        fn file() -> &'static str {
            "commands.yaml"
        }
    }

    impl Commands {
        fn add(&mut self, cmd: &str, body: &str) -> bool {
            use std::collections::hash_map::Entry::*;
            if let Vacant(e) = self.map.entry(cmd.to_string()) {
                e.insert(Command {
                    command: cmd.to_string(),
                    body: body.to_string(),
                    uses: 0,
                });
                return true;
            }
            false
        }

        fn update(&mut self, cmd: &str, body: &str) -> bool {
            self.map
                .get_mut(cmd)
                .map(|cmd| cmd.body = body.to_string())
                .is_some()
        }

        fn remove(&mut self, cmd: &str) -> Option<Command> {
            self.map.remove(cmd)
        }

        fn get_all_names(&self) -> impl Iterator<Item = &str> {
            self.map.keys().map(|c| &**c)
        }

        fn find(&mut self, cmd: &str) -> Option<&mut Command> {
            self.map.get_mut(cmd).map(|cmd| {
                cmd.uses += 1;
                cmd
            })
        }
    }

    pub struct UserDefined {
        commands: Commands,
    }

    impl UserDefined {
        pub async fn bind<R: Replier>() -> anyhow::Result<Bind<Self, R>> {
            let commands = shakey::data::load_yaml().await?;
            Bind::create::<responses::Responses>(Self { commands })?
                .bind(Self::add)?
                .bind(Self::update)?
                .bind(Self::remove)?
                .bind(Self::commands)?
                .listen(Self::listen)
        }

        // TODO support !alias

        fn add(&mut self, msg: &Message<impl Replier>, mut args: Arguments) -> impl Outcome {
            if !msg.is_from_elevated() {
                msg.problem(RequiresPermission {});
                return;
            }

            if !Self::check_body(msg, &args) {
                return;
            }

            if !args["command"].starts_with('!') {
                msg.problem(responses::InvalidSyntax {
                    error: "commands must start with !",
                });
                return;
            }

            let cmd = args.take("command");
            let body = args.take("body");

            if !self.commands.add(&cmd, &body) {
                msg.problem(responses::CommandExists { command: cmd });
                return;
            }

            msg.reply(responses::Added { command: cmd, body });

            self.save();
        }

        fn update(&mut self, msg: &Message<impl Replier>, mut args: Arguments) -> impl Outcome {
            if !msg.is_from_elevated() {
                msg.problem(RequiresPermission {});
                return;
            }

            if !Self::check_body(msg, &args) {
                return;
            }

            if !args["command"].starts_with('!') {
                msg.problem(responses::InvalidSyntax {
                    error: "commands must start with !",
                });
                return;
            }

            let cmd = args.take("command");
            let body = args.take("body");

            if !self.commands.update(&cmd, &body) {
                msg.problem(responses::CommandNotFound { command: cmd });
                return;
            }

            msg.reply(responses::Updated { command: cmd, body });

            self.save();
        }

        fn remove(&mut self, msg: &Message<impl Replier>, mut args: Arguments) -> impl Outcome {
            if !msg.is_from_elevated() {
                msg.problem(RequiresPermission {});
                return;
            }

            if !args["command"].starts_with('!') {
                msg.problem(responses::InvalidSyntax {
                    error: "commands must start with !",
                });
                return;
            }

            let cmd = args.take("command");
            if let Some(command) = self.commands.remove(&cmd) {
                msg.reply(responses::Removed {
                    command: cmd,
                    body: command.body,
                });
                return self.save();
            }

            msg.problem(responses::CommandNotFound {
                command: cmd.to_string(),
            });
        }

        fn commands(&mut self, msg: &Message<impl Replier>, args: Arguments) -> impl Outcome {
            const MAX_PER_LINE: usize = 10;
            let commands = self
                .commands
                .get_all_names()
                .join_multiline_max(MAX_PER_LINE);
            msg.reply(responses::Commands { commands })
        }

        fn listen(&mut self, msg: &Message<impl Replier>) -> impl Outcome {
            if let Some(cmd) = self.commands.find(&msg.data) {
                msg.reply(responses::Command {
                    body: cmd.body.clone(),
                });
                self.save()
            }
        }

        fn save(&self) {
            let commands = self.commands.clone();
            tokio::task::spawn(async move { shakey::data::save_yaml(&commands).await });
        }

        fn check_body(msg: &Message<impl Replier>, args: &Arguments) -> bool {
            if args.get("body").filter(|c| !c.trim().is_empty()).is_none() {
                msg.problem(responses::InvalidSyntax {
                    error: "the body cannot be empty",
                });
                return false;
            }
            true
        }
    }
}
