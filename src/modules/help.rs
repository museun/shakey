use crate::{
    ext::IterExt,
    global::GlobalItem,
    handler::{Bindable, Components},
    irc::Message,
    Arguments, Bind, Commands, Replier,
};

// TODO get rid of this type
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

crate::make_response! {
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

#[async_trait::async_trait]
impl<R: Replier> Bindable<R> for Help {
    type Responses = responses::Responses;
    async fn bind(_: &Components) -> anyhow::Result<Bind<Self, R>> {
        Bind::create(Self)?.bind(Self::help)
    }
}

impl Help {
    fn help(&mut self, msg: &Message<impl Replier>, args: Arguments) {
        const MAX_PER_LINE: usize = 10;

        let commands = Commands::get();
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
