use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc::{Sender, UnboundedReceiver},
};

use crate::{global::GlobalItem, handler::Reply, Message, Replier, Response, Templates};

use super::raw::{parse_line, Command, Line};

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

#[derive(Debug)]
pub struct Identity {
    pub display_name: String,
    pub user_id: u64,
}

pub async fn read_responses<R>(
    msg: Message<R>,
    mut recv: UnboundedReceiver<Reply<Box<dyn Response>>>,
    out: Sender<String>,
) where
    R: Replier,
{
    use crate::templates::Variant::Default as Irc;
    while let Some(resp) = recv.recv().await {
        let resp = resp
            .map(|resp| Templates::get().render(&resp, Irc))
            .transpose();

        let resp = match resp {
            Some(inner) => inner,
            None => continue,
        };

        let Message { sender, target, .. } = &msg;

        fn format_lines(left: String, right: &str) -> Vec<String> {
            right
                .lines()
                .map(|right| format!("{left}{right}\r\n"))
                .collect()
        }

        let data = match resp {
            Reply::Say(resp) => format_lines(format!("PRIVMSG {target} :"), &resp),
            Reply::Reply(resp) | Reply::Problem(resp) => {
                format_lines(format!("PRIVMSG {target} :{sender}: "), &resp)
            }
        };

        for line in data {
            if out.send(line).await.is_err() {
                break;
            }
        }
    }
}

pub async fn connect(addr: &str, name: &str, oauth: &str) -> anyhow::Result<TcpStream> {
    let mut stream = map_io_err(TcpStream::connect(addr).await)?;
    for cap in [
        "CAP REQ :twitch.tv/membership\r\n",
        "CAP REQ :twitch.tv/tags\r\n",
        "CAP REQ :twitch.tv/commands\r\n",
        &format!("PASS {oauth}\r\n"),
        &format!("NICK {name}\r\n"),
    ] {
        map_io_err(stream.write_all(cap.as_bytes()).await)?;
    }
    map_io_err(stream.flush().await)?;

    Ok(stream)
}

pub async fn join<A>(channel: &str, conn: A) -> anyhow::Result<()>
where
    A: AsyncWrite + Unpin + Send + Sized,
{
    let data = format!("JOIN {channel}\r\n");
    write_raw(&data, conn).await
}

pub async fn write_raw<A>(data: &str, mut conn: A) -> anyhow::Result<()>
where
    A: AsyncWrite + Unpin + Send + Sized,
{
    log::trace!("-> {}", data.escape_debug());
    map_io_err(conn.write_all(data.as_bytes()).await)?;
    map_io_err(conn.flush().await)
}

pub async fn wait_for_ready<A>(buf: &mut String, mut conn: A) -> anyhow::Result<Identity>
where
    A: AsyncBufRead + AsyncWrite + Unpin + Send + Sized,
{
    loop {
        if let Command::GlobalUserState {
            display_name,
            user_id,
        } = read_line(buf, &mut conn).await?.command
        {
            let display_name = display_name.to_string();
            log::debug!("ready: {display_name} ({user_id})");

            return Ok(Identity {
                display_name,
                user_id,
            });
        }
    }
}

pub async fn read_line<A>(buf: &mut String, mut conn: A) -> anyhow::Result<Line<'_>>
where
    A: AsyncBufRead + AsyncWrite + Unpin + Send + Sized,
{
    buf.clear();
    let buf = match map_io_err(conn.read_line(buf).await)? {
        0 => return Err(Eof.into()),
        n => &buf[..n],
    };
    log::trace!("<- {}", buf.escape_debug());

    let line = parse_line(buf)
        .map_err(|err| anyhow::anyhow!("cannot parse line: {err}. line: {}", buf.escape_debug()))?;

    match line.command {
        Command::Ping { token } => {
            let data = format!("PONG {token}\r\n");
            map_io_err(conn.write_all(data.as_bytes()).await)?;
            map_io_err(conn.flush().await)?
        }
        Command::Error { error } => anyhow::bail!("Twitch error: {error}"),
        _ => {}
    }
    Ok(line)
}
