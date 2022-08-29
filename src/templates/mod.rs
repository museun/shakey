mod variant;
use std::{collections::HashMap, path::Path, sync::Arc};

use tokio::sync::RwLock;
pub use variant::Variant;

mod environment;
pub use environment::{BorrowedEnv, Environment};

use crate::util::get_env_var;

use self::{parsed::Parsed, verify::Verifier};

mod parsed;

mod verify;

static TEMPLATES: tokio::sync::OnceCell<Arc<RwLock<Arc<Templates>>>> =
    tokio::sync::OnceCell::const_new();

pub async fn init_templates() -> std::io::Result<&'static Arc<RwLock<Arc<Templates>>>> {
    TEMPLATES
        .get_or_try_init(move || async move {
            let path = get_env_var("SHAKEN_TEMPLATES_PATH")?;
            Templates::load_from_yaml(path)
                .await
                .map(Arc::new)
                .map(RwLock::new)
                .map(Arc::new)
        })
        .await
}

pub async fn templates() -> Arc<Templates> {
    init_templates()
        .await
        .expect("initialization")
        .read()
        .await
        .clone()
}

pub async fn verify_templates() -> Verifier {
    Verifier {
        templates: templates().await,
    }
}

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

impl Templates {
    pub async fn load_from_yaml(path: impl AsRef<Path> + Send) -> std::io::Result<Self> {
        let data = tokio::fs::read_to_string(path).await?;
        serde_yaml::from_str(&data)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
    }

    pub fn render<T: crate::Response>(&self, resp: &T, variant: Variant) -> Option<String> {
        let parsed = self.find(resp.module(), resp.key(), variant)?;
        Some(parsed.apply(resp.as_environment()))
    }

    pub(crate) fn maybe_find(&self, module: &str, key: &str, variant: Variant) -> Option<&Parsed> {
        self.modules
            .get(module)?
            .entries
            .get(key)?
            .cache
            .get(&variant)
    }

    fn find(&self, module: &str, key: &str, variant: Variant) -> Option<&Parsed> {
        let map = &self.modules.get(module)?.entries.get(key)?.cache;
        match map.get(&variant) {
            Some(parsed) => Some(parsed),
            None => map.get(&Variant::Default),
        }
    }
}
