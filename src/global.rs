use std::sync::Arc;

use once_cell::sync::OnceCell;
use parking_lot::RwLock;

use crate::{handler::Commands, templates::Templates};

static COMMANDS: OnceCell<RwLock<Arc<Commands>>> = OnceCell::new();
static TEMPLATES: OnceCell<RwLock<Arc<Templates>>> = OnceCell::new();

pub static GLOBAL_COMMANDS: Global<'static, Commands> = Global(&COMMANDS);
pub static GLOBAL_TEMPLATES: Global<'static, Templates> = Global(&TEMPLATES);

pub trait GlobalItem: Sized + Send + Sync + 'static {
    fn description() -> &'static str;
    fn get() -> Arc<Self> {
        Self::get_static().get()
    }
    fn get_static() -> &'static Global<'static, Self>;
}

impl GlobalItem for Commands {
    fn get_static() -> &'static Global<'static, Self> {
        &GLOBAL_COMMANDS
    }

    fn description() -> &'static str {
        "Commands"
    }
}

impl GlobalItem for Templates {
    fn get_static() -> &'static Global<'static, Self> {
        &GLOBAL_TEMPLATES
    }

    fn description() -> &'static str {
        "Templates"
    }
}

#[derive(Copy, Clone)]
pub struct Global<'a, T>(pub(crate) &'a OnceCell<RwLock<Arc<T>>>);

impl<'a, T> Global<'a, T> {
    pub fn reset(&'static self)
    where
        T: Default,
    {
        if let Some(item) = self.0.get() {
            *item.write() = <Arc<T>>::default();
        }
    }

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
