use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use time::OffsetDateTime;

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

    fn join_multiline_max(self, max: usize) -> String {
        self.enumerate().fold(String::new(), |mut a, (i, c)| {
            if i > 0 && i % max == 0 {
                a.push('\n')
            }
            if !a.is_empty() {
                a.push(' ')
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

pub trait FormatTime {
    fn as_readable_time(&self) -> String;
}

impl FormatTime for time::Duration {
    fn as_readable_time(&self) -> String {
        format_seconds(self.as_seconds_f64() as _)
    }
}

impl FormatTime for std::time::Duration {
    fn as_readable_time(&self) -> String {
        format_seconds(self.as_secs())
    }
}

fn format_seconds(mut secs: u64) -> String {
    const TABLE: [(&str, u64); 4] = [
        ("days", 86400),
        ("hours", 3600),
        ("minutes", 60),
        ("seconds", 1),
    ];

    fn pluralize(s: &str, n: u64) -> String {
        format!("{} {}", n, if n > 1 { s } else { &s[..s.len() - 1] })
    }

    let mut time = vec![];
    for (name, d) in &TABLE {
        let div = secs / d;
        if div > 0 {
            time.push(pluralize(name, div));
            secs -= d * div;
        }
    }

    let len = time.len();
    if len > 1 {
        if len > 2 {
            for segment in time.iter_mut().take(len - 2) {
                segment.push(',')
            }
        }
        time.insert(len - 1, "and".into())
    }
    time.join(" ")
}

pub trait WithCommas: Sized {
    fn with_commas(self) -> String;
}

impl WithCommas for u64 {
    fn with_commas(self) -> String {
        use std::fmt::Write as _;
        fn comma(n: u64, s: &mut String) {
            if n < 1000 {
                write!(s, "{}", n).unwrap();
                return;
            }
            comma(n / 1000, s);
            write!(s, ",{:03}", n % 1000).unwrap();
        }

        let mut buf = String::new();
        comma(self, &mut buf);
        buf
    }
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

pub trait DurationSince: Sized {
    fn duration_since_now_utc_human(self) -> String {
        self.duration_since_human(OffsetDateTime::now_utc())
    }

    fn duration_since_human(self, later: OffsetDateTime) -> String;
}

impl DurationSince for time::OffsetDateTime {
    fn duration_since_human(self, later: OffsetDateTime) -> String {
        std::time::Duration::try_from(later - self)
            .expect("valid time")
            .as_readable_time()
    }
}
