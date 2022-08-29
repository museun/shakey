use std::sync::Arc;

use crate::irc;

use super::{Callable, Error, Outcome};

type BoxedHandler = Box<dyn Fn(&irc::Message<'_>)>;

pub struct Bind<T: 'static> {
    this: Arc<parking_lot::Mutex<T>>,
    handlers: Vec<BoxedHandler>,
}

impl<'i, T: 'static> Callable<irc::Message<'i>> for Bind<T> {
    type Outcome = ();

    fn call_func(&mut self, msg: &irc::Message<'i>) -> Self::Outcome {
        for handlers in &mut self.handlers {
            (handlers)(msg);
        }
    }
}

impl<T: 'static> Bind<T> {
    pub fn create(this: T) -> Self {
        Self {
            this: Arc::new(parking_lot::Mutex::new(this)),
            handlers: vec![],
        }
    }

    pub fn bind<O: Outcome + 'static>(
        mut self,
        pattern: &'static str,
        handler: fn(&mut T, &irc::Message<'_>) -> O,
    ) -> Self {
        let this = Arc::clone(&self.this);
        let this = move |msg: &irc::Message| {
            if &*msg.data != pattern {
                return;
            }

            let this = &mut *this.lock();
            if let Some(error) = handler(this, msg).into_error() {
                msg.problem(Error { error })
            }
        };

        self.handlers.push(Box::new(this) as _);
        self
    }
}
