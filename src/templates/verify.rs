use crate::{Response, Templates, Variant};
use std::{collections::HashSet, sync::Arc};

#[derive(Debug)]
pub struct Error {
    module: &'static str,
    key: &'static str,
    variants: Vec<Variant>,
    kind: ErrorKind,
}

impl Error {
    fn new(ty: &dyn Response, variants: Vec<Variant>, kind: ErrorKind) -> Self {
        Self {
            module: ty.module(),
            key: ty.key(),
            variants,
            kind,
        }
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    MissingTemplate,
    EmptyStruct,
    NoVariants,
    Mismatched {
        keys: Vec<String>,
        available: Vec<&'static str>,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            module,
            key,
            variants,
            ..
        } = self;

        write!(f, "`{module}.{key}@{variants:?}` ")?;

        use ErrorKind::*;
        match &self.kind {
            MissingTemplate => {
                write!(f, "template is missing")
            }
            EmptyStruct => {
                write!(f, "the `Response` struct had no fields")
            }
            NoVariants => {
                write!(f, "no variants were provided")
            }
            Mismatched { keys, available } => {
                write!(
                    f,
                    "extra keys were found in template: {keys:?}. available: {available:?}",
                )
            }
        }
    }
}
impl std::error::Error for Error {}

pub struct Verifier {
    pub templates: Arc<Templates>,
}

impl Verifier {
    pub fn verify<T>(self, variants: &[Variant]) -> Result<Self, Error>
    where
        T: Response + Default + serde::Serialize,
    {
        let this = T::default();
        if variants.is_empty() {
            return Err(Error::new(&this, vec![], ErrorKind::NoVariants));
        }

        let fields = crate::ser::get_fields::<T>();
        if fields.is_empty() {
            return Err(Error::new(&this, variants.to_vec(), ErrorKind::EmptyStruct));
        }

        let fields_set = fields.iter().copied().collect::<HashSet<_>>();
        for variant in variants {
            let keys = match self
                .templates
                .maybe_find(this.module(), this.key(), *variant)
            {
                Some(parsed) => &parsed.keys,
                None => {
                    return Err(Error::new(
                        &this,
                        vec![*variant],
                        ErrorKind::MissingTemplate,
                    ))
                }
            };

            let diff = keys
                .iter()
                .map(|s| &**s)
                .collect::<HashSet<_>>()
                .difference(&fields_set)
                .map(|s| s.to_string())
                .collect::<Vec<_>>();

            if !diff.is_empty() {
                return Err(Error::new(
                    &this,
                    vec![*variant],
                    ErrorKind::Mismatched {
                        keys: diff,
                        available: fields,
                    },
                ));
            }
        }

        Ok(self)
    }
}
