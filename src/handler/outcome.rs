pub trait Outcome: Sized {
    fn into_error(self) -> Option<String> {
        None
    }
}

impl Outcome for () {}

impl<E> Outcome for Result<(), E>
where
    E: std::fmt::Display,
{
    fn into_error(self) -> Option<String> {
        match self {
            Ok(..) => None,
            Err(resp) => Some(resp.to_string()),
        }
    }
}
