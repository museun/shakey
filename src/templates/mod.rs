use std::collections::HashMap;

mod macros;

mod variant;
pub use variant::Variant;

mod environment;
pub use environment::{BorrowedEnv, Environment, RegisterResponse};

mod parsed;
use parsed::Parsed;

mod verify;
pub use verify::{add_to_registry, reset_registry};

use crate::{
    data::{Interest, InterestPath},
    handler::Response,
};

#[derive(Debug, serde::Deserialize)]
struct Module {
    #[serde(flatten)]
    entries: HashMap<String, Entries>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(transparent)]
struct Entries {
    cache: HashMap<Variant, Parsed>,
}

#[derive(Default, Debug, serde::Deserialize)]
#[serde(transparent)]
pub struct Templates {
    modules: HashMap<String, Module>,
}

impl Interest for Templates {
    fn module() -> InterestPath<&'static str> {
        InterestPath::Root
    }

    fn file() -> &'static str {
        "templates.yaml"
    }
}

impl Templates {
    pub fn render<T>(&self, resp: &T, variant: Variant) -> Option<String>
    where
        T: Response + 'static,
    {
        let parsed = match self.try_find(resp.module(), resp.key(), variant) {
            Some(parsed) => parsed,
            None => {
                log::error!("cannot find template: {}", resp as &dyn Response);
                return None;
            }
        };

        Some(parsed.apply(resp.as_environment()))
    }

    fn get_entries(&self, module: &str, key: &str) -> Option<&Entries> {
        self.modules.get(module)?.entries.get(key)
    }

    pub fn variants_for<'a>(
        &'a self,
        module: &str,
        key: &str,
    ) -> Option<impl Iterator<Item = Variant> + 'a> {
        Some(self.get_entries(module, key)?.cache.keys().copied())
    }

    pub(crate) fn maybe_find(&self, module: &str, key: &str, variant: Variant) -> Option<&Parsed> {
        self.get_entries(module, key)?.cache.get(&variant)
    }

    fn try_find(&self, module: &str, key: &str, variant: Variant) -> Option<&Parsed> {
        let map = &self.get_entries(module, key)?.cache;
        match map.get(&variant) {
            Some(parsed) => Some(parsed),
            None => map.get(&Variant::Default),
        }
    }
}
