use anyhow::Context;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use serde::Serialize;

use crate::{ext::IterExt, global::GlobalItem, handler::Response, Templates};
use std::collections::{BTreeSet, HashMap};

#[derive(Default)]
struct ResponseRegistry {
    responses: HashMap<(&'static str, &'static str), Box<dyn Response>>,
}

static RESPONSE_REGISTRY: Lazy<Mutex<ResponseRegistry>> =
    Lazy::new(|| Mutex::new(ResponseRegistry::default()));

pub fn reset_registry() {
    std::mem::take(&mut *RESPONSE_REGISTRY.lock());
}

pub fn add_to_registry<T>() -> anyhow::Result<()>
where
    T: Response + Default + Serialize + 'static,
{
    use std::collections::hash_map::Entry::*;

    let this = T::default();
    match RESPONSE_REGISTRY
        .lock()
        .responses
        .entry((this.module(), this.key()))
    {
        Occupied(..) => {
            anyhow::bail!("response already exists: {}", &this as &dyn Response)
        }
        Vacant(entry) => {
            verify(T::default())?;
            entry.insert(Box::new(this));
            Ok(())
        }
    }
}

fn verify<T>(response: T) -> anyhow::Result<()>
where
    T: Response + Serialize + 'static,
{
    let fields = crate::get_fields::get_fields_for(&response)
        .into_iter()
        .collect::<BTreeSet<_>>();

    let templates = Templates::get();

    anyhow::ensure!(
        templates
            .get_entries(response.module(), response.key())
            .is_some(),
        "cannot find any templates for: {}",
        &response as &dyn Response
    );

    for variant in templates
        .variants_for(response.module(), response.key())
        .with_context(|| {
            anyhow::anyhow!(
                "cannot find any variants for: {}",
                &response as &dyn Response,
            )
        })?
    {
        let keys = match templates.maybe_find(response.module(), response.key(), variant) {
            Some(parsed) => &parsed.keys,
            None => {
                anyhow::bail!(
                    "missing template for: {}@{:?}",
                    &response as &dyn Response,
                    variant
                );
            }
        };

        let left = keys.iter().map(|s| &**s).collect::<BTreeSet<_>>();
        anyhow::ensure!(
            left.difference(&fields).count() == 0,
            "mismatched variables in: {}@{:?} found [{}], have [{}]",
            &response as &dyn Response,
            variant,
            left.iter().join_with(", "),
            fields.iter().join_with(", ")
        );
    }

    Ok(())
}
