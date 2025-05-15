use std::{
    env,
    ffi::{OsStr, OsString},
};

use anyhow::Context;
use nu_plugin_core::CommunicationMode;

use super::{io, Plugin};

// TODO: remove anyhow

pub async fn serve_cli(plugin: impl Plugin) -> anyhow::Result<()> {
    let args: Vec<OsString> = env::args_os().skip(1).collect();

    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        log::error!("invalid calling convention");
        anyhow::bail!("invalid calling convention");
    }

    let mode = if args[0] == "--stdio" && args.len() == 1 {
        // --stdio always supported.
        CommunicationMode::Stdio
    } else if args[0] == "--local-socket" && args.len() == 2 {
        CommunicationMode::LocalSocket((&args[1]).into())
    } else {
        log::error!(
            "{}: This plugin must be run from within Nushell. See `plugin add --help` for details \
            on how to use plugins.",
            env::current_exe()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| "plugin".into())
        );
        log::error!(
            "If you are running from Nushell, this plugin may be incompatible with the \
            version of nushell you are using."
        );
        anyhow::bail!("invalid usage");
    };

    serve(plugin, mode).await
}

pub async fn serve(plugin: impl Plugin, mode: CommunicationMode) -> anyhow::Result<()> {
    match mode {
        CommunicationMode::Stdio => {
            let rx = crate::rt::io::stdin()?;
            let tx = crate::rt::io::stdout()?;

            let mut manager = super::manager::open::<io::Json, _, _, _>(plugin, tx, rx).await?;
            manager.run().await
        }
        CommunicationMode::LocalSocket(name) => {
            todo!()
            // use crate::rt::io::EventedSource;
            // use interprocess::local_socket as ls;
            // use ls::traits::*;
            // let name = interpret_local_socket_name(&name)?;
            // let stream = ls::Stream::connect(name)?;
            // let (rx, tx) = stream.split();

            // let mut manager = super::manager::open::<io::Json, _, _, _>(
            //     plugin,
            //     EventedSource::new(rx),
            //     EventedSource::new(tx),
            // )
            // .await?;
            // manager.run().await
        }
    }
}

#[cfg(unix)]
fn interpret_local_socket_name(
    name: &OsStr,
) -> Result<interprocess::local_socket::Name, std::io::Error> {
    use interprocess::local_socket::{GenericFilePath, ToFsName};

    name.to_fs_name::<GenericFilePath>()
}

/// Interpret a local socket name for use with `interprocess`.
#[cfg(windows)]
fn interpret_local_socket_name(
    name: &OsStr,
) -> Result<interprocess::local_socket::Name, std::io::Error> {
    use interprocess::local_socket::{GenericNamespaced, ToNsName};

    name.to_ns_name::<GenericNamespaced>()
}
