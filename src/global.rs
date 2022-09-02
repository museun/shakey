use std::sync::Arc;

use once_cell::sync::OnceCell;
use parking_lot::RwLock;

use crate::{handler::Commands, templates::Templates};

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

static COMMANDS: OnceCell<RwLock<Arc<Commands>>> = OnceCell::new();
static TEMPLATES: OnceCell<RwLock<Arc<Templates>>> = OnceCell::new();

pub static GLOBAL_COMMANDS: Global<'static, Commands> = Global(&COMMANDS);
pub static GLOBAL_TEMPLATES: Global<'static, Templates> = Global(&TEMPLATES);

#[derive(Copy, Clone)]
pub struct Global<'a, T: GlobalItem>(pub(crate) &'a OnceCell<RwLock<Arc<T>>>);

impl<'a, T: GlobalItem> Global<'a, T> {
    pub fn initialize(&'static self, item: Arc<T>)
    where
        T: Default,
    {
        if let Some(inner) = self.0.get() {
            let _ = std::mem::replace(&mut *inner.write(), item);
            return;
        }

        self.0.get_or_init(move || RwLock::new(item));
    }

    pub fn get(&'static self) -> Arc<T> {
        self.0.get().expect("initialization").read().clone()
    }
}
