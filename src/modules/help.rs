use crate::{
    global::GlobalItem,
    handler::{Bindable, Components},
    templates::LimitedVec,
    Arguments, Bind, Commands, Message, Replier,
};

crate::make_response! {
    module: "help"

    struct ListCommands {
        commands: crate::templates::LimitedVec<String>
    } is "list_commands"

    struct SpecificCommandNoAlias {
        command: String,
        usage: Option<String>,
        description: String,
    } is "specific_command_no_alias"

    struct SpecificCommand {
        command: String,
        usage: Option<String>,
        description: String,
        aliases: crate::templates::LimitedVec<String>,
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
        let commands = Commands::get();
        let cmd = match args.get("command") {
            Some(cmd) => cmd,
            None => {
                let list = commands.command_names().map(ToString::to_string);
                let commands = LimitedVec::new(10, list);
                msg.say(responses::ListCommands { commands });
                return;
            }
        };

        if let Some(cmd) = commands.find_by_name(cmd) {
            let usage = (!cmd.args.usage.is_empty()).then(|| cmd.args.usage.to_string());

            let command = cmd.command.clone();
            let description = cmd.description.clone();

            if cmd.aliases.is_empty() {
                msg.say(responses::SpecificCommandNoAlias {
                    command,
                    usage,
                    description,
                });
                return;
            }

            let list = cmd.aliases.iter().map(ToString::to_string);
            let aliases = LimitedVec::new(10, list);

            msg.say(responses::SpecificCommand {
                command,
                usage,
                description,
                aliases,
            });
            return;
        }

        msg.say(responses::UnknownCommand {
            command: cmd.to_string(),
        })
    }
}
