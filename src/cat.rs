use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use nu_plugin::{Plugin, PluginCommand};
use nu_protocol::{
    IntoInterruptiblePipelineData, LabeledError, ListStream, PipelineData, Signature, SyntaxShape,
};
use tokio::{io::AsyncWriteExt, net::TcpSocket};

pub struct CatCommand;

impl PluginCommand for CatCommand {
    type Plugin = crate::Plugin;

    fn name(&self) -> &str {
        "net cat"
    }

    fn signature(&self) -> nu_protocol::Signature {
        // TODO: search terms
        Signature::new(self.name())
            .switch("listen", "Use NC is listening mode. Instead of sending data, `net cat` will send data to the configured address", Some('l'))
            .switch("udp", "", Some('u'))
            .switch("v4", "Prefer IPv4", Some('4'))
            .switch("v6", "Prefer IPv6", Some('6'))
            // .named("port", SyntaxShape::Int, "Specifies the source port to use. Subject to privlege restrictions and availability.", Some('p'))
            .named(
                "recv-buffer-size",
                SyntaxShape::OneOf(vec![SyntaxShape::Int, SyntaxShape::Filesize]),
                "Overrides the kernel sizing for the buffer for this socket. In receive mode, this controls the",
                None
            )
            .required("addr", SyntaxShape::String, "The target address, like '127.0.0.1' or 'fe80:::'")
            .required("port", SyntaxShape::Int, "The target port")
    }

    fn description(&self) -> &str {
        "net cat desc"
    }

    fn run(
        &self,
        plugin: &Self::Plugin,
        engine: &nu_plugin::EngineInterface,
        call: &nu_plugin::EvaluatedCall,
        input: nu_protocol::PipelineData,
    ) -> Result<nu_protocol::PipelineData, LabeledError> {
        enum Proto {
            V4,
            V6,
        }

        let user_proto = match (call.has_flag("v4")?, call.has_flag("v6")?) {
            (false, false) => Ok(None), // If unspecified, infer from address parsing
            (true, false) => Ok(Some(Proto::V4)),
            (false, true) => Ok(Some(Proto::V6)),
            (true, true) => {
                let v4_span = call.get_flag_span("v4");
                let v6_span = call.get_flag_span("v6");

                let mut err = LabeledError::new("Cannot specify --v4 alongside --v6");

                if let Some(s) = v4_span {
                    err = err.with_label("--v4 set here", s);
                }
                if let Some(s) = v6_span {
                    err = err.with_label("--v6 set here", s);
                }

                Err(err)
            }
        }?;

        let raw_addr = call.positional[0].as_str()?;

        // TODO: add spans to all of this

        let addr = match user_proto {
            None => raw_addr.parse::<IpAddr>().map_err(|e| {
                LabeledError::new(format!("Could not parse '{raw_addr}' as an address: {e}"))
            })?,
            Some(Proto::V4) => raw_addr
                .parse::<Ipv4Addr>()
                .map_err(|e| {
                    LabeledError::new(format!(
                        "Could not parse '{raw_addr}' as an Ipv4 address: {e}"
                    ))
                })?
                .into(),
            Some(Proto::V6) => raw_addr
                .parse::<Ipv6Addr>()
                .map_err(|e| {
                    LabeledError::new(format!(
                        "Could not parse '{raw_addr}' as an Ipv6 address: {e}"
                    ))
                })?
                .into(),
        };

        let port = call.positional[1]
            .as_int()?
            .try_into()
            .map_err(|e| LabeledError::new(format!("Could not coerce into port {e}")))?;

        let proto = match addr {
            IpAddr::V4(_) => Proto::V4,
            IpAddr::V6(_) => Proto::V6,
        };

        let sock_addr = SocketAddr::new(addr, port);

        let input = input.into_pipeline_data(call.head, engine.signals().clone());

        let socket = match proto {
            Proto::V4 => TcpSocket::new_v4(),
            Proto::V6 => TcpSocket::new_v6(),
        }
        .map_err(|e| LabeledError::new(format!("Failed to open TCP socket: {e}")))?;

        let (rx, tx) = crossbeam_channel::unbounded();

        let _ = plugin.rt.spawn(async move {
            let stream = socket.connect(sock_addr).await.unwrap();
            // stream.write_all()
        });

        Ok(PipelineData::ListStream(
            ListStream::new(
                std::iter::from_fn(move || tx.recv().ok()),
                call.head,
                engine.signals().clone(),
            ),
            None,
        ))
    }
}
