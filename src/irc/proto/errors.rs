pub fn map_io_err<T>(err: Result<T, std::io::Error>) -> anyhow::Result<T> {
    use std::io::ErrorKind::*;
    err.map_err(|err| match err.kind() {
        UnexpectedEof => Eof.into(),
        ConnectionRefused | ConnectionReset | ConnectionAborted => Connection.into(),
        TimedOut => Timeout.into(),
        _ => err.into(),
    })
}

macro_rules! make_error {
    ($($ident:ident)*) => {
        $(
            #[derive(Debug)]
            pub struct $ident;
            impl std::fmt::Display for $ident {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    std::fmt::Debug::fmt(self, f)
                }
            }
            impl std::error::Error for $ident {}
        )*
    };
}

make_error! {
    Eof
    Timeout
    Connection
}
