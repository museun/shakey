use super::Environment;

#[derive(Debug)]
pub struct Parsed {
    pub input: String,
    pub keys: Vec<String>,
    cached_keys: Vec<String>,
}

impl<'de> serde::Deserialize<'de> for Parsed {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let s = <std::borrow::Cow<'_, str>>::deserialize(deserializer)?;
        Self::parse(&s).map_err(D::Error::custom)
    }
}

impl Parsed {
    pub fn parse(input: &str) -> Result<Self, &'static str> {
        Self::find_keys(input).map(|keys| Self {
            input: input.into(),
            cached_keys: keys
                .iter()
                .map(|key| format!("${{{}}}", key))
                .map(Into::into)
                .collect(),
            keys,
        })
    }

    pub fn find_keys(input: &str) -> Result<Vec<String>, &'static str> {
        let (mut heads, mut tails) = (vec![], vec![]);

        let mut last = false;
        let mut iter = input.char_indices().peekable();
        while let Some((pos, ch)) = iter.next() {
            match (ch, iter.peek()) {
                ('$', Some((_, '{'))) => {
                    last = true;
                    heads.push(pos);
                    iter.next();
                }

                ('{', ..) if last => return Err("nested templates aren't allowed"),
                ('}', ..) if last => {
                    tails.push(pos);
                    last = false;
                }
                _ => {}
            }
        }

        if heads.len() != tails.len() {
            return Err("variable isn't terminated");
        }

        heads
            .into_iter()
            .zip(tails)
            .map(|(head, tail)| {
                if tail < head + 3 {
                    return Err("variable was empty");
                }
                assert!(tail > head, "tail must be after the head");
                Ok(input[head + 2..tail].into())
            })
            .collect()
    }

    pub(super) fn apply(&self, env: impl Environment) -> String {
        let mut temp = self.input.to_string();
        for (key, fkey) in self.keys.iter().zip(self.cached_keys.iter()) {
            if let Some(val) = env.resolve(key) {
                temp = temp.replace(&**fkey, &val);
            }
        }
        temp.shrink_to_fit();
        temp
    }
}
