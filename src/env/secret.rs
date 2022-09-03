use std::{borrow::Cow, path::Path};

#[derive(Clone)]
pub struct Secret<T = String> {
    key: Cow<'static, str>,
    value: T,
}

impl<T> Secret<T> {
    pub fn new_key_value(key: impl Into<Cow<'static, str>>, value: T) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

impl<T> Secret<T>
where
    T: Default,
{
    pub fn new_key(key: impl Into<Cow<'static, str>>) -> Self {
        Self {
            key: key.into(),
            value: T::default(),
        }
    }
}

impl<T> std::ops::Deref for Secret<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl AsRef<Path> for Secret<String> {
    fn as_ref(&self) -> &Path {
        self.value.as_ref()
    }
}

impl<T> AsRef<str> for Secret<T>
where
    T: AsRef<str>,
{
    fn as_ref(&self) -> &str {
        self.value.as_ref()
    }
}

impl<T> Secret<T> {
    pub fn into_value(self) -> T {
        self.value
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.key)
    }
}

impl<'de, T> serde::Deserialize<'de> for Secret<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let key = <Cow<'static, str>>::deserialize(deserializer)?;
        let value = std::env::var(&*key)
            .map_err(|_| format!("expected environment variable '{key}' to exist"))
            .map_err(D::Error::custom)?
            .parse()
            .map_err(D::Error::custom)?;
        Ok(Self { key, value })
    }
}

impl serde::Serialize for Secret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.key)
    }
}
