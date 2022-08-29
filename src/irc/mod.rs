use tokio::io::{AsyncWriteExt, BufStream};

use crate::{
    ext::{Either, FutureExt},
    irc::{
        lower::Command,
        proto::{connect, join, read_line, read_responses, wait_for_ready},
    },
    util::get_env_var,
    Callable,
};

mod proto;

mod message;
pub use self::message::Message;

mod lower;

pub async fn run<const N: usize, const M: usize>(
    mut handlers: [&mut (dyn for<'i> Callable<Message<'i>, Outcome = ()> + Send); N],
    channels: [&str; M],
) -> std::io::Result<()> {
    let stream = connect(
        "irc.chat.twitch.tv:6667",
        &get_env_var("SHAKEN_TWITCH_NAME")?,
        &get_env_var("SHAKEN_TWITCH_OAUTH_TOKEN")?,
    )
    .await?;

    let mut stream = BufStream::new(stream);

    let mut buf = String::with_capacity(1024);
    let identity = wait_for_ready(&mut buf, &mut stream).await?;
    eprintln!(
        "connected, our identity: {} | id: {}",
        identity.display_name, identity.user_id
    );

    for channel in channels {
        join(channel, &mut stream).await?;
    }

    let (write_tx, mut write_rx) = tokio::sync::mpsc::channel(32);

    loop {
        match read_line(&mut buf, &mut stream)
            .select(write_rx.recv())
            .await
        {
            Either::Left(msg) => {
                if let msg @ Command::Privmsg { .. } = msg?.command {
                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

                    let msg = Message::new(&msg, tx);
                    for handler in &mut handlers {
                        eprintln!("dispatching");
                        // outcome is always () here
                        handler.call_func(&msg);
                    }

                    tokio::spawn(read_responses(msg.as_owned(), rx, write_tx.clone()));
                }
            }

            Either::Right(Some(data)) => {
                eprintln!("?? {}", data.escape_debug());
                stream.write_all(data.as_bytes()).await?;
                stream.flush().await?;
            }

            _ => break Ok(()),
        }
    }
}
