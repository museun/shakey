use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    str::FromStr,
};

#[derive(Default, Debug, Clone)]
pub struct Arguments {
    pub map: HashMap<String, String>,
}

impl Arguments {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.get(key).map(|s| &**s)
    }

    pub fn get_parsed<T>(&self, key: &str) -> Option<Result<T, T::Err>>
    where
        T: FromStr,
    {
        self.get(key).map(<str>::parse)
    }
}

impl std::ops::Index<&str> for Arguments {
    type Output = str;

    fn index(&self, key: &str) -> &Self::Output {
        self.get(key)
            .unwrap_or_else(|| panic!("{key} should exist"))
    }
}

#[derive(Default, Clone, Debug)]
pub struct ExampleArgs {
    pub usage: Box<str>,
    pub args: Box<[ArgType]>,
}

impl<'de> serde::Deserialize<'de> for ExampleArgs {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let s = <Cow<'_, str>>::deserialize(deserializer)?;
        Self::parse(&s).map_err(D::Error::custom)
    }
}

impl ExampleArgs {
    pub fn contains(&self, arg: &Kind) -> bool {
        self.args.iter().any(|ArgType { kind, .. }| kind == arg)
    }
}

impl ExampleArgs {
    pub fn extract(&self, mut input: &str) -> Match<HashMap<String, String>> {
        if input.is_empty() {
            if self.contains(&Kind::Required) {
                return Match::Required;
            }
            if !self.args.is_empty()
                && (!self.contains(&Kind::Optional) && !self.contains(&Kind::Variadic))
            {
                return Match::NoMatch;
            }
        }

        if !input.is_empty() && self.args.is_empty() {
            return Match::NoMatch;
        }

        use Kind::*;
        let mut map = HashMap::new();
        for ArgType { key, kind } in &*self.args {
            match (kind, input.find(' ')) {
                (Required | Optional, None) | (Variadic, ..) => {
                    if !input.is_empty() {
                        map.insert(key.into(), input.into());
                    }
                    break;
                }
                (.., Some(pos)) => {
                    let (head, tail) = input.split_at(pos);
                    map.insert(key.into(), head.into());
                    input = tail.trim();
                }
            }
        }

        Match::Match(map)
    }

    pub fn parse(input: &str) -> anyhow::Result<Self> {
        // <required> <optional?> <rest..>
        let mut seen = HashSet::new();
        let mut args = vec![];

        for token in input.split_ascii_whitespace() {
            let mut append = |arg: &[_]| {
                let data = &token[1..arg.len() + 1];
                anyhow::ensure!(seen.insert(data), "duplicate argument found: {data}");
                Ok(data.into())
            };

            let all_alpha = move |s: &[u8]| {
                s.iter()
                    .all(|d| matches!(d, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
            };

            let arg = match token.as_bytes() {
                [b'<', arg @ .., b'.', b'.', b'>'] if all_alpha(arg) => ArgType {
                    key: append(arg)?,
                    kind: Kind::Variadic,
                },
                [b'<', arg @ .., b'?', b'>'] if all_alpha(arg) => ArgType {
                    key: append(arg)?,
                    kind: Kind::Optional,
                },
                [b'<', arg @ .., b'>'] if all_alpha(arg) => ArgType {
                    key: append(arg)?,
                    kind: Kind::Required,
                },
                // TODO report invalid patterns
                // TODO report invalid characters in keys
                _ => continue,
            };

            let done = arg.kind == Kind::Variadic;
            args.push(arg);

            if done {
                break;
            }
        }

        Ok(Self {
            usage: Box::from(input),
            args: args.into(),
        })
    }
}

#[derive(Debug)]
pub enum Match<T> {
    Required,
    Match(T),
    NoMatch,
}

#[derive(Clone, Debug)]
pub struct ArgType {
    pub key: String,
    pub kind: Kind,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Required,
    Optional,
    Variadic,
}
