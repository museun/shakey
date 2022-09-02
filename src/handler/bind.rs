use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use anyhow::Context;
use heck::ToSnekCase;

use crate::{
    data::{Interest, InterestPath},
    global::GlobalItem,
    irc, responses, Arguments, Callable, Outcome, RegisterResponse, Replier,
};

use super::{
    arguments::{ExampleArgs, Match},
    Bindable,
};

type BoxedHandler<R> = Box<dyn Fn(&irc::Message<R>) + Send + Sync>;

pub struct Bind<T, R>
where
    T: Send + Sync + 'static,
    R: Replier + 'static,
{
    this: Arc<parking_lot::Mutex<T>>,
    handlers: Vec<BoxedHandler<R>>,
}

impl<T, R> Callable<irc::Message<R>> for Bind<T, R>
where
    T: Send + Sync + 'static,
    R: Replier + 'static,
{
    type Outcome = ();

    fn call_func(&mut self, msg: &irc::Message<R>) -> Self::Outcome {
        for handlers in &mut self.handlers {
            (handlers)(msg);
        }
    }
}

impl<T, R> Bind<T, R>
where
    T: Send + Sync + 'static,
    R: Replier + Send + Sync + 'static,
{
    pub fn create(this: T) -> anyhow::Result<Self>
    where
        T: Bindable<R>,
    {
        T::Responses::register()?;
        Ok(Self {
            this: Arc::new(parking_lot::Mutex::new(this)),
            handlers: vec![],
        })
    }

    pub fn bind<O, F>(mut self, handler: F) -> anyhow::Result<Self>
    where
        O: Outcome + 'static,
        F: Fn(&mut T, &irc::Message<R>, Arguments) -> O + Send + Sync + 'static,
        F: Copy + 'static,
    {
        let (module, key) = Self::make_keyable::<F>();
        log::trace!("adding handler: {module}.{key}");
        Commands::get()
            .find(&module, &key)
            .with_context(|| anyhow::anyhow!("cannot find {module}.{key}"))?;

        let this = Arc::clone(&self.this);
        let this = move |msg: &irc::Message<R>| {
            let cmd = Commands::get();
            let cmd = cmd.find(&module, &key).expect("command should exist");

            let map = match Self::parse_command(cmd, msg) {
                Some(map) => map,
                None => return,
            };

            let this = &mut *this.lock();
            let outcome = handler(this, msg, map);

            if outcome.is_error() {
                if let Some(error) = outcome.into_error() {
                    msg.problem(responses::Error { error })
                }
                return;
            }

            if let Some(task) = outcome.into_task() {
                let msg = msg.clone();
                let fut = async move {
                    if let Ok(Err(err)) = task.await {
                        msg.problem(responses::Error {
                            error: err.to_string(),
                        })
                    }
                };
                tokio::spawn(fut);
            }
        };

        self.handlers.push(Box::new(this) as _);
        Ok(self)
    }

    pub fn listen<O, F>(mut self, handler: F) -> anyhow::Result<Self>
    where
        O: Outcome + 'static,
        F: Fn(&mut T, &irc::Message<R>) -> O + Send + Sync + 'static + Copy,
    {
        let this = Arc::clone(&self.this);
        let this = move |msg: &irc::Message<R>| {
            let this = &mut *this.lock();
            if let Some(error) = handler(this, msg).into_error() {
                msg.problem(responses::Error { error })
            }
        };

        self.handlers.push(Box::new(this) as _);
        Ok(self)
    }

    pub fn into_callable(self) -> Box<dyn Callable<irc::Message<R>, Outcome = ()>> {
        Box::new(self) as _
    }

    fn parse_command(cmd: &Command, msg: &irc::Message<R>) -> Option<Arguments> {
        if !cmd.has_args() && cmd.is_command_match(&msg.data) {
            return Some(Arguments::default());
        }

        let tail = cmd.without_command(&msg.data)?.unwrap_or_default();
        match cmd.args.extract(tail.trim()) {
            Match::Match(map) => Some(Arguments { map }),
            Match::NoMatch => None,
            Match::Required => {
                msg.problem(responses::InvalidUsage {
                    usage: format!("{} {}", cmd.command, cmd.args.usage),
                });
                None
            }
        }
    }

    fn make_keyable<F>() -> (String, String) {
        fn fix(s: &str) -> &str {
            let s = s.split_once('<').map(|(head, _)| head).unwrap_or(s);
            s.rsplit_once("::").map(|(_, tail)| tail).unwrap_or(s)
        }

        let module = std::any::type_name::<T>();
        let key = std::any::type_name::<F>();

        let module = fix(module);
        let key = fix(key);

        (module.to_snek_case(), key.to_snek_case())
    }
}

// TODO this should be cheaply clonable
#[derive(Debug, serde::Deserialize)]
pub struct Command {
    pub command: String,
    pub description: String,
    #[serde(default)]
    pub aliases: BTreeSet<String>,
    #[serde(default)]
    pub args: ExampleArgs,
}

impl Command {
    pub fn is_command_match(&self, query: &str) -> bool {
        std::iter::once(&*self.command)
            .chain(self.aliases.iter().map(|s| &**s))
            .any(|c| c == query)
    }

    pub fn without_command<'a>(&'a self, query: &'a str) -> Option<Option<&str>> {
        // this breaks if the command has a space in it
        let mut iter = query.splitn(2, ' ');
        let head = iter.next()?;

        if !self.is_command_match(head) {
            return None;
        }

        Some(iter.next())
    }

    pub const fn has_args(&self) -> bool {
        !self.args.args.is_empty()
    }
}

#[derive(Debug, serde::Deserialize)]
struct Module {
    #[serde(flatten)]
    entries: HashMap<String, Command>,
}

#[derive(Default, Debug, serde::Deserialize)]
#[serde(transparent)]
pub struct Commands {
    modules: HashMap<String, Module>,
}

impl Interest for Commands {
    fn module() -> InterestPath<&'static str> {
        InterestPath::Root
    }

    fn file() -> &'static str {
        "commands.yaml"
    }
}

impl Commands {
    pub fn find_by_name(&self, query: &str) -> Option<&Command> {
        self.modules
            .values()
            .filter_map(|module| {
                module
                    .entries
                    .values()
                    .find(|cmd| cmd.command == query || cmd.aliases.contains(query))
            })
            .next()
    }

    pub fn command_names(&self) -> impl Iterator<Item = &str> {
        self.modules.values().flat_map(|module| {
            module.entries.values().flat_map(|cmd| {
                std::iter::once(&*cmd.command).chain(cmd.aliases.iter().map(|c| &**c))
            })
        })
    }

    pub fn find(&self, module: &str, key: &str) -> Option<&Command> {
        self.modules.get(module)?.entries.get(key)
    }
}
