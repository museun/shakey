use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub trait IterExt
where
    Self: Sized + Iterator,
    Self::Item: AsRef<str>,
{
    fn join_with(self, sp: &str) -> String {
        self.fold(String::new(), |mut a, c| {
            if !a.is_empty() {
                a.push_str(sp)
            }
            a.push_str(c.as_ref());
            a
        })
    }
}

impl<I> IterExt for I
where
    I: Iterator,
    I::Item: AsRef<str>,
{
}

pub trait FutureExt: Future + Sized {
    fn select<F>(self, other: F) -> Select<Self, F>;
}

impl<T> FutureExt for T
where
    T: Future + Sized,
{
    fn select<F>(self, other: F) -> Select<Self, F> {
        Select {
            left: self,
            right: other,
        }
    }
}

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

pin_project_lite::pin_project! {
    pub struct Select<L, R> {
        #[pin] left: L,
        #[pin] right: R,
    }
}

impl<L: Future, R: Future> Future for Select<L, R> {
    type Output = Either<L::Output, R::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        macro_rules! poll {
            ($($expr:ident => $map:ident)*) => {
                $(if let Poll::Ready(val) = this.$expr.poll(cx) {
                    return Poll::Ready(Either::$map(val))
                })*
                return Poll::Pending;
            };
        }

        if fastrand::bool() {
            poll! {
                left => Left
                right => Right
            }
        }

        poll! {
            right => Right
            left => Left
        }
    }
}
