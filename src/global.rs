use std::sync::Arc;

use once_cell::sync::OnceCell;
use parking_lot::RwLock;

use crate::{handler::Commands, templates::Templates};

static COMMANDS: OnceCell<RwLock<Arc<Commands>>> = OnceCell::new();
static TEMPLATES: OnceCell<RwLock<Arc<Templates>>> = OnceCell::new();

pub static GLOBAL_COMMANDS: Global<'static, Commands> = Global(&COMMANDS);
pub static GLOBAL_TEMPLATES: Global<'static, Templates> = Global(&TEMPLATES);

pub struct Global<'a, T>(pub(crate) &'a OnceCell<RwLock<Arc<T>>>);
impl<'a, T> Global<'a, T> {
    pub fn initialize(&'static self, item: Arc<T>) -> &'static RwLock<Arc<T>> {
        self.0.get_or_init(move || RwLock::new(item))
    }

    pub fn update(&'static self, item: Arc<T>) {
        *self.0.get().expect("initialization").write() = item;
    }

    pub fn get(&'static self) -> Arc<T> {
        self.0.get().expect("initialization").read().clone()
    }
}

pub fn templates() -> Arc<Templates> {
    GLOBAL_TEMPLATES.get()
}
pub fn commands() -> Arc<Commands> {
    GLOBAL_COMMANDS.get()
}
