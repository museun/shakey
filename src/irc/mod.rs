use tokio::io::BufStream;

use crate::{
    env::EnvVar,
    ext::{Either, FutureExt},
    handler::SharedCallable,
};

mod proto;
use proto::{connect, join, read_line, read_responses, wait_for_ready, write_raw};

mod message;
pub use message::Message;

mod raw;
use raw::Command;

pub mod errors {
    pub use super::proto::{Connection, Eof, Timeout};
}

pub async fn run(handlers: Vec<SharedCallable>) -> anyhow::Result<()> {
    let channels = crate::env::SHAKEN_TWITCH_CHANNELS::get()?;
    let channels = channels.split(',').collect::<Vec<_>>();
    anyhow::ensure!(!channels.is_empty(), "channels cannot be empty");

    let stream = connect(
        &crate::env::SHAKEN_TWITCH_ADDRESS::get()?,
        &crate::env::SHAKEN_TWITCH_NAME::get()?,
        &crate::env::SHAKEN_TWITCH_OAUTH_TOKEN::get()?,
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
        log::info!("joining: {channel}");
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
                if let msg @ Command::Privmsg {
                    ref sender,
                    ref target,
                    ref data,
                    ..
                } = msg.command
                {
                    log::debug!("[{}] {}: {}", target, sender, data);

                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                    let irc_msg = Message::new(msg);
                    let msg = crate::Message::twitch(irc_msg, tx);
                    for handler in &handlers {
                        // outcome is always () here
                        (handler)(msg.clone());
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
