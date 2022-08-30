use std::collections::HashMap;

pub trait Environment {
    fn resolve(&self, key: &str) -> Option<String>;
}

impl<'f> Environment for BorrowedEnv<'f> {
    fn resolve(&self, key: &str) -> Option<String> {
        self.map.get(key).map(|s| s.to_string())
    }
}

#[derive(Default)]
pub struct BorrowedEnv<'f> {
    map: HashMap<&'static str, &'f dyn std::fmt::Display>,
}

impl<'f> BorrowedEnv<'f> {
    pub fn insert(mut self, key: &'static str, val: &'f dyn std::fmt::Display) -> Self {
        self.map.insert(key, val);
        self
    }
}

pub trait RegisterResponse {
    fn register() -> anyhow::Result<()>;
}

#[macro_export]
macro_rules! borrowed_env {
    ($($key:expr => $value:expr),* $(,)?) => {
        $crate::BorrowedEnv::default()$(.insert($key, $value))*
    };
}

#[macro_export]
macro_rules! make_response {
    (module: $module:literal $(
        struct $name:ident $(< $($lifetimes:lifetime),* >)? {
            $($key:ident: $val:ty),* $(,)?
        } is $path:literal
    )*) => {
        pub mod responses {
            $($crate::make_response!(@inner
                module: $module;
                key: $path;
                struct $name $(< $($lifetimes),* >)? {
                    $($key: $val),*
                }
            );)*

            pub struct Responses;
            impl $crate::RegisterResponse for Responses {
                fn register() -> anyhow::Result<()> {
                    $($crate::templates::add_to_registry::<$name>()?;)*
                    Ok(())
                }
            }
        }
    };

    (@inner
        module: $module:literal;
        key: $path:literal;
        struct $name:ident $(< $($lifetimes:lifetime),* >)? {
            $($key:ident: $val:ty),* $(,)?
        }
    ) => {


        #[derive(Debug, Clone, Default, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
        pub struct $name $(< $($lifetimes),* >)? {
            $( pub $key: $val, )*
        }

        impl $(< $($lifetimes),* >)? $crate::Response for $name $(< $($lifetimes),* >)? {
            fn as_environment(&self) -> $crate::BorrowedEnv<'_> {
                $crate::borrowed_env! {
                    $(stringify!($key) => &self.$key),*
                }
            }

            fn module(&self) -> &'static str {
                $module
            }

            fn key(&self) -> &'static str {
                $path
            }
        }
    };
}
