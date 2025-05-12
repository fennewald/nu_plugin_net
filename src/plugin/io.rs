use futures::{
    io::BufReader, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, Stream, StreamExt,
};
use nu_plugin_protocol::{PluginInput, PluginOutput};
use nu_protocol::ShellError;

use crate::channel::Sender;

// TODO: clean up signature
pub(super) async fn consume<E, W, R>(
    mut w: W,
    r: R,
    errs: Sender<ShellError>,
) -> Result<
    (
        Sender<PluginOutput>,
        impl Stream<Item = Result<PluginInput, ShellError>>,
    ),
    ShellError,
>
where
    E: AsyncEncoder,
    W: AsyncWrite + Unpin + 'static,
    R: AsyncRead + Unpin,
{
    tell_encoding::<E, _>(&mut w).await?;

    let rx = E::consume_stream(r);
    let tx = E::spawn_writer(w, errs);

    Ok((tx, rx))
}

pub trait AsyncEncoder: 'static {
    const NAME: &str;

    fn consume_stream(
        r: impl AsyncRead + Unpin,
    ) -> impl Stream<Item = Result<PluginInput, ShellError>> + Unpin;

    fn encode(data: &PluginOutput) -> Result<Vec<u8>, ShellError>;

    fn spawn_writer(
        mut w: impl AsyncWrite + Unpin + 'static,
        errs: Sender<ShellError>,
    ) -> Sender<PluginOutput> {
        let (tx, mut rx) = crate::channel::with_capacity(128);
        crate::rt::spawn(async move {
            let report_error = |err| {
                log::error!("{err}");
                errs.send(err);
            };

            while let Some(it) = rx.recv().await {
                if let Err(e) = write_msg::<Self, _>(&mut w, it).await {
                    report_error(e);
                }
            }

            log::debug!("writer task exiting");
        });

        tx
    }
}

async fn write_msg<E, W>(w: &mut W, obj: PluginOutput) -> Result<(), ShellError>
where
    W: AsyncWrite + Unpin + 'static,
    E: AsyncEncoder + ?Sized + 'static,
{
    log::debug!("Sending: {:?}", obj);
    let start = std::time::Instant::now();
    let buffer = E::encode(&obj)?;
    w.write_all(&buffer)
        .await
        .map_err(|e| ShellError::NushellFailed {
            msg: format!("failed to send data on socket: {e}"),
        })?;
    w.flush().await.map_err(|e| ShellError::NushellFailed {
        msg: format!("failed to flush socket: {e}"),
    })?;
    log::debug!("sent in {:?}", start.elapsed());
    Ok(())
}

async fn tell_encoding<E, W>(w: &mut W) -> Result<(), ShellError>
where
    E: AsyncEncoder,
    W: AsyncWrite + Unpin,
{
    let name = E::NAME.as_bytes();
    let mut encoding = Vec::with_capacity(name.len() + 1);
    encoding.push(name.len() as u8);
    encoding.extend_from_slice(name);

    w.write_all(&encoding)
        .await
        .map_err(|e| ShellError::NushellFailed { msg: e.to_string() })?;
    w.flush()
        .await
        .map_err(|e| ShellError::NushellFailed { msg: e.to_string() })?;
    Ok(())
}

pub struct Json;

impl AsyncEncoder for Json {
    const NAME: &str = "json";

    fn consume_stream(
        r: impl AsyncRead + Unpin,
    ) -> impl Stream<Item = Result<PluginInput, ShellError>> + Unpin {
        BufReader::new(r).lines().map(|it| match it {
            Ok(s) => serde_json::from_str(&s).map_err(|e| ShellError::PluginFailedToDecode {
                msg: format!("failed to decode '{:?}': {}", s, e),
            }),
            Err(e) => Err(ShellError::NushellFailed {
                msg: format!("Failed to get line from input: {e}"),
            }),
        })
    }

    fn encode(data: &PluginOutput) -> Result<Vec<u8>, ShellError> {
        serde_json::to_vec(data)
            .map(|mut b| {
                b.push(b'\n');
                b
            })
            .map_err(|e| ShellError::PluginFailedToEncode { msg: e.to_string() })
    }
}
