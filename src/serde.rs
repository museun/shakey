use std::{borrow::Cow, str::FromStr, time::Duration};

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

pub fn simple_human_time<'de, D>(deser: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let s = <Cow<'_, str>>::deserialize(deser)?;
    // TODO validate
    let dur = s
        .split_terminator(',')
        .flat_map(|s| s.trim().split_once(' '))
        .fold(0_u64, |dur, (head, tail)| {
            let d = u64::from_str_radix(head, 10).unwrap_or(0);
            dur + match tail {
                "hour" => d * 60 * 60,
                "minute" => d * 60,
                "second" => d,
                _ => return dur,
            }
        });

    Ok(Duration::from_secs(dur))
}
