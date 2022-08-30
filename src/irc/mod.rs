use tokio::io::BufStream;

use crate::{
    ext::{Either, FutureExt},
    irc::{
        lower::Command,
        proto::{connect, join, read_line, read_responses, wait_for_ready, write_raw},
    },
    util::get_env_var,
    Callable, Response,
};

mod proto;
pub use proto::errors;

mod message;
pub use self::message::{Message, Replier};

mod lower;

type BoxedCallable = Box<dyn Callable<Message<Box<dyn Response>>, Outcome = ()>>;

pub async fn run<const N: usize>(mut handlers: [BoxedCallable; N]) -> anyhow::Result<()> {
    let channels = get_env_var("SHAKEN_TWITCH_CHANNELS")?;
    let channels = channels.split(',').collect::<Vec<_>>();
    anyhow::ensure!(!channels.is_empty(), "channels cannot be empty");

    let stream = connect(
        &get_env_var("SHAKEN_TWITCH_ADDRESS")?,
        &get_env_var("SHAKEN_TWITCH_NAME")?,
        &get_env_var("SHAKEN_TWITCH_OAUTH_TOKEN")?,
    )
    .await?;

    let mut stream = BufStream::new(stream);

    let mut buf = String::with_capacity(1024);
    let identity = wait_for_ready(&mut buf, &mut stream).await?;
    log::info!(
        "connected, our identity: {} | id: {}",
        identity.display_name,
        identity.user_id
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
            Either::Left(Err(err)) => break Err(err),

            Either::Left(Ok(msg)) => {
                if let msg @ Command::Privmsg { .. } = msg.command {
                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

                    let msg = Message::new(msg, tx);
                    for handler in &mut handlers {
                        // outcome is always () here
                        handler.call_func(&msg);
                    }

                    tokio::spawn(read_responses(msg, rx, write_tx.clone()));
                }
            }

            Either::Right(Some(data)) => {
                write_raw(&data, &mut stream).await?;
            }

            _ => break Ok(()),
        }
    }
}
