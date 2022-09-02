use std::{
    borrow::Cow,
    collections::{BTreeSet, HashMap},
};

use crate::ext::IterExt;

pub trait Environment {
    fn resolve(&self, key: &str) -> Option<String>;
}

impl<'f> Environment for BorrowedEnv<'f> {
    fn resolve(&self, key: &str) -> Option<String> {
        self.map.get(key).map(|s| s.show())
    }
}

#[derive(Default)]
pub struct BorrowedEnv<'f> {
    map: HashMap<&'static str, &'f dyn Show>,
}

impl<'f> BorrowedEnv<'f> {
    pub fn insert(mut self, key: &'static str, val: &'f dyn Show) -> Self {
        self.map.insert(key, val);
        self
    }
}

pub trait RegisterResponse {
    fn register() -> anyhow::Result<()>;
}

pub trait Show {
    fn show(&self) -> String;
}

macro_rules! show_impl {
    ($($ty:ty)*) => {
        $(
            impl Show for $ty {
                fn show(&self) -> String {
                    self.to_string()
                }
            }
        )*
    };
}

show_impl! {
    i8 i16 i32 i64 isize
    u8 u16 u32 u64 usize
    f32 f64 bool
}

impl Show for &'static str {
    fn show(&self) -> String {
        self.to_string()
    }
}

impl Show for str {
    fn show(&self) -> String {
        self.to_string()
    }
}

impl Show for String {
    fn show(&self) -> String {
        self.clone()
    }
}

impl<'a> Show for Cow<'a, str> {
    fn show(&self) -> String {
        self.to_string()
    }
}

impl<T> Show for &T
where
    T: Show,
{
    fn show(&self) -> String {
        T::show(&**self)
    }
}

impl<T> Show for Option<T>
where
    T: Show,
{
    fn show(&self) -> String {
        self.as_ref().map(Show::show).unwrap_or_default()
    }
}

#[derive(Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LimitedVec<T> {
    inner: Vec<T>,
    max: usize,
}

impl<T> LimitedVec<T>
where
    T: Show,
{
    pub fn new(max: usize, list: impl IntoIterator<Item = T>) -> Self {
        Self {
            inner: list.into_iter().collect(),
            max,
        }
    }
}

impl<T> Show for LimitedVec<T>
where
    T: Show,
{
    fn show(&self) -> String {
        self.inner
            .iter()
            .map(<_>::show)
            .join_multiline_max(self.max)
    }
}

impl<T> Show for Vec<T>
where
    T: Show,
{
    fn show(&self) -> String {
        show_iter('[', ']', self.iter())
    }
}

impl<T> Show for BTreeSet<T>
where
    T: Show,
{
    fn show(&self) -> String {
        show_iter('{', '}', self.iter())
    }
}

fn show_iter<T: Show>(head: char, tail: char, iter: impl Iterator<Item = T>) -> String {
    let mut out = iter
        .map(|s| s.show())
        .enumerate()
        .fold(String::from(head), |mut a, (i, c)| {
            if i > 0 {
                a.push_str(", ");
            }
            a.push_str(&c);
            a
        });
    out.push(tail);
    out
}
