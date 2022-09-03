use std::{future::Future, path::PathBuf, time::Duration};

pub async fn watch_file<Fut>(
    path: impl Into<PathBuf> + Send,
    sleep: Duration,
    modification: Duration,
    update: impl Fn(PathBuf) -> Fut + Sync + Send,
) -> anyhow::Result<()>
where
    Fut: Future<Output = anyhow::Result<()>> + Send,
{
    let path = path.into();

    let md = match tokio::fs::metadata(&path).await {
        Ok(md) => md,
        Err(err) => {
            log::error!("cannot read metadata for {}, {err}", path.display());
            anyhow::bail!("{err}")
        }
    };

    let mut last = md.modified()?;

    loop {
        tokio::time::sleep(sleep).await;

        let md = match tokio::fs::metadata(&path).await {
            Ok(md) => md,
            Err(err) => {
                log::error!("cannot read metadata for {}, {err}", path.display());
                continue;
            }
        };

        if md
            .modified()
            .ok()
            .and_then(|md| md.duration_since(last).ok())
            .filter(|&dur| dur >= modification)
            .is_some()
        {
            log::info!("file {} was modified", path.display());

            if let Err(err) = (update)(path.clone()).await {
                log::warn!("cannot update file: {err}");
                continue;
            }
            last = md
                .modified()
                .expect("already checked that the metadata exists")
        }
    }
}
