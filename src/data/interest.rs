use std::path::{Path, PathBuf};

#[non_exhaustive]
pub enum InterestPath<T> {
    Nested(T),
    Root,
}

pub trait Interest: Default {
    fn module() -> InterestPath<&'static str>;
    fn file() -> &'static str;

    fn get_path(root: &Path) -> PathBuf {
        match Self::module() {
            InterestPath::Nested(path) => root.join(path).join(Self::file()),
            InterestPath::Root => root.join(Self::file()),
        }
    }
}
