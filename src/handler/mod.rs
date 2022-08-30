mod outcome;
pub use outcome::Outcome;

mod callable;
pub use callable::Callable;

mod bind;
pub use bind::{Bind, Commands};

mod response;
pub use response::Response;

mod reply;
pub use reply::Reply;

mod arguments;
pub use arguments::{Arguments, ExampleArgs};

// why is this in that module?
pub use crate::irc::Replier;
