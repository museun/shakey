use std::{borrow::Cow, str::FromStr};

use serde::{de::Error, Deserialize, Deserializer};
use time::{format_description::FormatItem, macros::format_description, OffsetDateTime};

pub fn assume_utc_date_time<'de, D>(deser: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    const FORMAT: &[FormatItem<'static>] = format_description!(
        "[year]-[month]-[day]T\
        [hour]:[minute]:[second]Z\
        [offset_hour sign:mandatory][offset_minute]"
    );

    let s = <Cow<'_, str>>::deserialize(deser)? + "+0000";
    OffsetDateTime::parse(&s, &FORMAT).map_err(Error::custom)
}

pub fn from_str<'de, D, T>(deser: D) -> Result<T, D::Error>
where
    T: FromStr,
    T::Err: std::fmt::Display,
    D: Deserializer<'de>,
{
    <Cow<'_, str>>::deserialize(deser)?
        .parse()
        .map_err(Error::custom)
}

pub mod simple_human_time {
    use std::{borrow::Cow, time::Duration};

    use crate::ext::IterExt;

    pub fn serialize<S>(dt: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ::serde::Serializer,
    {
        let mut secs = dt.as_secs();
        const TABLE: [(&str, u64); 3] = [
            ("hours", 60 * 60), //
            ("minutes", 60),
            ("seconds", 1),
        ];

        let mut time = Vec::new();
        for (name, d) in &TABLE {
            let div = secs / d;
            if div > 0 {
                time.push((name, div));
                secs -= d * div;
            }
        }

        let out = time
            .into_iter()
            .map(|(name, div)| format!("{div} {name}"))
            .join_with(", ");

        serializer.serialize_str(&out)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: ::serde::Deserializer<'de>,
    {
        use ::serde::Deserialize as _;
        let data = <Cow<'_, str>>::deserialize(deserializer)?;
        // TODO validate
        let dur = data
            .split_terminator(',')
            .flat_map(|s| s.trim().split_once(' '))
            .fold(0_u64, |dur, (head, tail)| {
                let d = head.parse::<u64>().unwrap_or(0);
                dur + match tail {
                    "hours" => d * 60 * 60,
                    "minutes" => d * 60,
                    "seconds" => d,
                    _ => return dur,
                }
            });

        Ok(Duration::from_secs(dur))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn round_trip() {
            let input = [r#""10 hours, 3 minutes, 39 seconds""#];

            #[derive(::serde::Serialize, ::serde::Deserialize)]
            #[serde(transparent)]
            struct W(#[serde(with = "super")] std::time::Duration);

            for left in input {
                let dt = serde_json::from_str::<W>(left).unwrap();
                let right = serde_json::to_string(&dt).unwrap();
                assert_eq!(left, right);
            }
        }
    }
}
