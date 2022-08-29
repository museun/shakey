use crate::BorrowedEnv;

pub trait Response: Send {
    fn module(&self) -> &'static str;
    fn key(&self) -> &'static str;
    fn as_environment(&self) -> BorrowedEnv<'_>;
}

impl<T> Response for &T
where
    T: Response + Send + Sync,
{
    fn module(&self) -> &'static str {
        Response::module(&**self)
    }

    fn key(&self) -> &'static str {
        Response::key(&**self)
    }

    fn as_environment(&self) -> BorrowedEnv<'_> {
        Response::as_environment(&**self)
    }
}

impl Response for Box<dyn Response> {
    fn module(&self) -> &'static str {
        Response::module(&**self)
    }

    fn key(&self) -> &'static str {
        Response::key(&**self)
    }

    fn as_environment(&self) -> BorrowedEnv<'_> {
        Response::as_environment(&**self)
    }
}
