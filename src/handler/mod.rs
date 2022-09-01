use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

mod outcome;
pub use outcome::{MaybeTask, Outcome};

mod callable;
pub use callable::Callable;

mod bind;
pub use bind::{Bind, Commands};

mod response;
pub use response::Response;

mod reply;
pub use reply::Reply;

mod arguments;
pub use arguments::Arguments;

mod replier;
pub use replier::Replier;

use crate::RegisterResponse;

#[derive(Default, Clone)]
pub struct Components {
    map: Arc<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl Components {
    pub fn register<T: Any + Send + Sync + 'static>(mut self, item: T) -> Self {
        assert!(Arc::get_mut(&mut self.map)
            .expect("single ownership")
            .insert(TypeId::of::<T>(), Box::new(item))
            .is_none());
        self
    }

    pub fn get_ref<T: Any + Send + Sync + 'static>(&self) -> &T {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|inner| inner.downcast_ref::<T>())
            .unwrap_or_else(|| {
                panic!(
                    "expected key `{}` to exist in components",
                    std::any::type_name::<T>()
                )
            })
    }

    pub fn get<T: Any + Send + Sync + 'static + Clone>(&self) -> T {
        self.get_ref::<T>().clone()
    }
}

#[async_trait::async_trait]
pub trait Bindable<R: Replier>: Sized + Send + Sync + 'static {
    type Responses: RegisterResponse;
    async fn bind(components: &Components) -> anyhow::Result<Bind<Self, R>>;
}

pub type BoxedCallable<R = Box<dyn Response>> =
    Box<dyn Callable<crate::irc::Message<R>, Outcome = ()>>;
