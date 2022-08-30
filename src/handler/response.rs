use crate::BorrowedEnv;

pub trait Response: Send + Sync {
    fn as_environment(&self) -> BorrowedEnv<'_>;
    fn module(&self) -> &'static str;
    fn key(&self) -> &'static str;
}

impl<T> Response for &T
where
    T: Response + Send + Sync,
{
    fn as_environment(&self) -> BorrowedEnv<'_> {
        Response::as_environment(&**self)
    }

    fn module(&self) -> &'static str {
        Response::module(&**self)
    }

    fn key(&self) -> &'static str {
        Response::key(&**self)
    }
}

impl Response for Box<dyn Response> {
    fn as_environment(&self) -> BorrowedEnv<'_> {
        Response::as_environment(&**self)
    }
    fn module(&self) -> &'static str {
        Response::module(&**self)
    }

    fn key(&self) -> &'static str {
        Response::key(&**self)
    }
}

impl std::fmt::Display for dyn Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (module, key) = (self.module(), self.key());
        write!(f, "{module}.{key}")
    }
}
