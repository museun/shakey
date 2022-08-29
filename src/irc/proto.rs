use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    sync::mpsc::{Sender, UnboundedReceiver},
};

use crate::{handler::Reply, templates::templates, Response};

use super::{
    lower::{parse_line, Command, Line},
    message::Message,
};

pub async fn read_responses(
    msg: Message<'static>,
    mut recv: UnboundedReceiver<Reply<Box<dyn Response>>>,
    out: Sender<String>,
) {
    use crate::Variant::Default as Irc;
    while let Some(resp) = recv.recv().await {
        let resp = {
            let t = templates().await;
            resp.map(|resp| t.render(&resp, Irc)).transpose()
        };

        let resp = match resp {
            Some(inner) => inner,
            None => {
                eprintln!("cannot render template");
                continue;
            }
        };

        eprintln!("rendered: {resp:?}");

        let Message { sender, target, .. } = &msg;

        let data = match resp {
            Reply::Say(resp) | Reply::Problem(resp) => format!("PRIVMSG {target} :{resp}\r\n"),
            Reply::Reply(resp) => format!("PRIVMSG {target} :{sender}: {resp}\r\n"),
        };

        if out.send(data).await.is_err() {
            break;
        }
    }
}

pub async fn connect(addr: &str, name: &str, oauth: &str) -> std::io::Result<TcpStream> {
    let mut stream = TcpStream::connect(addr).await?;
    for cap in [
        "CAP REQ :twitch.tv/membership\r\n",
        "CAP REQ :twitch.tv/tags\r\n",
        "CAP REQ :twitch.tv/commands\r\n",
        &format!("PASS {oauth}\r\n"),
        &format!("NICK {name}\r\n"),
    ] {
        stream.write_all(cap.as_bytes()).await?;
    }
    stream.flush().await?;

    Ok(stream)
}

pub async fn join<A>(channel: &str, mut conn: A) -> std::io::Result<()>
where
    A: AsyncWrite + Unpin + Send + Sized,
{
    let data = format!("JOIN {channel}\r\n");
    eprintln!("joining: {channel}");
    conn.write_all(data.as_bytes()).await?;
    conn.flush().await
}

#[derive(Debug)]
pub struct Identity {
    pub display_name: String,
    pub user_id: u64,
}

pub async fn wait_for_ready<A>(buf: &mut String, mut conn: A) -> std::io::Result<Identity>
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
            eprintln!("ready: {display_name} ({user_id})");

            return Ok(Identity {
                display_name,
                user_id,
            });
        }
    }
}

pub async fn read_line<A>(buf: &mut String, mut conn: A) -> std::io::Result<Line<'_>>
where
    A: AsyncBufRead + AsyncWrite + Unpin + Send + Sized,
{
    buf.clear();
    let buf = match conn.read_line(buf).await? {
        0 => return Err(std::io::ErrorKind::UnexpectedEof.into()),
        n => &buf[..n],
    };
    let map_err = |err| std::io::Error::new(std::io::ErrorKind::Other, err);
    let line = parse_line(buf).map_err(map_err)?;
    match line.command {
        Command::Ping { token } => {
            let data = format!("PONG {token}\r\n");
            conn.write_all(data.as_bytes()).await?;
            conn.flush().await?
        }
        Command::Error { error } => return Err(map_err(error)),
        _ => {}
    }
    Ok(line)
}
