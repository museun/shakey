use std::collections::HashMap;

use crate::{
    data::Interest,
    ext::IterExt,
    handler::{Bindable, Components},
    irc::Message,
    responses::RequiresPermission,
    Arguments, Bind, Outcome, Replier,
};

crate::make_response! {
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
    fn module() -> Option<&'static str> {
        Some("user_defined")
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

#[async_trait::async_trait]
impl<R: Replier> Bindable<R> for UserDefined {
    type Responses = responses::Responses;

    async fn bind(_: &Components) -> anyhow::Result<Bind<Self, R>> {
        let this = Self {
            commands: crate::data::load_yaml().await?,
        };

        Bind::create(this)?
            .bind(Self::add)?
            .bind(Self::update)?
            .bind(Self::remove)?
            .bind(Self::commands)?
            .listen(Self::listen)
    }
}

impl UserDefined {
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

    fn commands(&mut self, msg: &Message<impl Replier>, _: Arguments) -> impl Outcome {
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
        tokio::task::spawn(async move { crate::data::save_yaml(&commands).await });
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
