#[derive(Debug)]
pub enum Reply<T> {
    Say(T),
    Reply(T),
    Problem(T),
}

impl<T> Reply<Option<T>> {
    pub fn transpose(self) -> Option<Reply<T>> {
        match self {
            Self::Say(inner) => inner.map(Reply::Say),
            Self::Reply(inner) => inner.map(Reply::Reply),
            Self::Problem(inner) => inner.map(Reply::Problem),
        }
    }
}

impl<T> Reply<T> {
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Reply<U> {
        match self {
            Self::Say(val) => Reply::Say(map(val)),
            Self::Reply(val) => Reply::Reply(map(val)),
            Self::Problem(val) => Reply::Problem(map(val)),
        }
    }

    pub const fn inner(&self) -> &T {
        match self {
            Self::Say(val) | Self::Reply(val) | Self::Problem(val) => val,
        }
    }
}
