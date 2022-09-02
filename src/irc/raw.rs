use std::collections::HashMap;

#[derive(Debug)]
pub struct Line<'a> {
    #[allow(dead_code)]
    pub line: &'a str,
    pub command: Command<'a>,
}

#[derive(Copy, Clone, Debug)]
pub enum Command<'a> {
    Ping {
        token: &'a str,
    },
    Error {
        error: &'a str,
    },
    GlobalUserState {
        display_name: &'a str,
        user_id: u64,
    },
    Privmsg {
        tags: Option<&'a str>,
        sender: &'a str,
        target: &'a str,
        data: &'a str,
    },
    Ignored,
}

pub fn parse_line(input: &str) -> Result<Line<'_>, &'static str> {
    fn as_tag_map(input: &str) -> HashMap<&str, &str> {
        input
            .split(';')
            .filter_map(|s| s.split_once('='))
            .map(|(k, v)| (k.trim(), v.trim()))
            .collect()
    }
    fn tags<'a>(input: &mut &'a str) -> Option<&'a str> {
        let (head, tail) = input.split_once(' ')?;
        *input = tail;
        Some(&head[1..])
    }
    fn prefix<'a>(input: &mut &'a str) -> Option<&'a str> {
        let (head, tail) = input[1..].split_once(' ')?;
        *input = tail;
        head.split_terminator('!').next()
    }
    fn command<'a>(input: &mut &'a str) -> &'a str {
        let (head, tail) = input.split_at(input.find(' ').unwrap_or(input.len()));
        *input = tail;
        head
    }
    fn args<'a>(input: &mut &'a str) -> Vec<&'a str> {
        input
            .split_once(':')
            .map(|(head, tail)| {
                *input = tail;
                head.split_ascii_whitespace().collect()
            })
            .unwrap_or_default()
    }
    fn data<'a>(input: &mut &'a str) -> Option<&'a str> {
        Some(input.trim_end()).filter(|s| !s.is_empty())
    }

    let line = input;
    let raw = &mut input.trim();

    let tags = raw.starts_with('@').then(|| tags(raw)).flatten();
    let prefix = raw.starts_with(':').then(|| prefix(raw)).flatten();
    let command = command(raw);
    let args = args(raw);
    let data = data(raw);

    let command = match command {
        "PING" => Command::Ping {
            token: data.ok_or("missing token")?,
        },
        "ERROR" => Command::Error {
            error: data.ok_or("missing message")?,
        },
        "PRIVMSG" => Command::Privmsg {
            tags,
            sender: prefix.ok_or("missing prefix")?,
            target: args.first().ok_or("missing target")?,
            data: data.ok_or("missing data")?,
        },
        "GLOBALUSERSTATE" => {
            let tags = tags
                .map(as_tag_map)
                .ok_or("tags attached to that message")?;
            Command::GlobalUserState {
                display_name: tags.get("display-name").ok_or("missing display-name tag")?,
                user_id: tags
                    .get("user-id")
                    .and_then(|s| s.parse().ok())
                    .ok_or("missing user-id tag")?,
            }
        }
        _ => Command::Ignored,
    };

    Ok(Line { line, command })
}
