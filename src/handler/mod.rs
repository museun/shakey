use crate::make_response;

mod outcome;
use outcome::Outcome;

mod callable;
pub use callable::Callable;

mod bind;
pub use bind::Bind;

mod response;
pub use response::Response;

mod reply;
pub use reply::Reply;

make_response!(
    module: "system";
    key: "command_error";
    struct Error {
        error: String,
    }
);
