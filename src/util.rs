use std::sync::Arc;

pub fn get_env_var(key: &str) -> std::io::Result<String> {
    eprintln!("loading: {key}");
    std::env::var(key)
        .map_err(|_| format!("expected '{key}' to exist in env"))
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
}

#[derive(Debug, Clone)]
pub enum ArcCow<'a> {
    Borrowed(&'a str),
    Owned(Arc<str>),
}

impl std::ops::Deref for ArcCow<'_> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(s) => s,
            Self::Owned(s) => &**s,
        }
    }
}

impl<'a> std::fmt::Display for ArcCow<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Borrowed(s) => s,
            Self::Owned(s) => &**s,
        };
        f.write_str(s)
    }
}

impl<'a> ArcCow<'a> {
    pub fn into_owned(self) -> ArcCow<'static> {
        match self {
            Self::Borrowed(inner) => ArcCow::Owned(Arc::from(inner)),
            Self::Owned(inner) => ArcCow::Owned(inner),
        }
    }
}
