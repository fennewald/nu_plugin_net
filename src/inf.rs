use nu_plugin::{EngineInterface, EvaluatedCall, SimplePluginCommand};
use nu_protocol::{LabeledError, Record, Signature, Span, Type, Value};
use pnet::{datalink::NetworkInterface, ipnetwork::IpNetwork};

/// A command for listing network interfaces and their IP addresses
pub struct InterfacesCommand;

fn map_ip(network: IpNetwork, span: Span) -> Value {
    let mut out = Record::with_capacity(3);

    out.push("addr", Value::string(network.ip().to_string(), span));
    out.push(
        "type",
        Value::string(
            match network {
                IpNetwork::V4(_) => "v4",
                IpNetwork::V6(_) => "v6",
            },
            span,
        ),
    );
    out.push("prefix", Value::int(network.prefix() as i64, span));

    Value::record(out, span)
}

fn map_flags(inf: &NetworkInterface, span: Span) -> Value {
    let mut out = Record::with_capacity(5);

    out.push("is_up", Value::bool(inf.is_up(), span));
    out.push("is_broadcast", Value::bool(inf.is_broadcast(), span));
    out.push("is_loopback", Value::bool(inf.is_loopback(), span));
    out.push(
        "is_point_to_point",
        Value::bool(inf.is_point_to_point(), span),
    );
    out.push("is_multicast", Value::bool(inf.is_multicast(), span));

    Value::record(out, span)
}

/// Maps a network interface to a nushell record
fn map_interface(inf: NetworkInterface, span: Span) -> Value {
    let mut o = Record::with_capacity(6);

    // Measure flags first so that we can partially move out of inf
    let flags = map_flags(&inf, span);

    o.push("name", Value::string(inf.name, span));
    o.push("description", Value::string(inf.description, span));
    o.push("if_index", Value::int(inf.index as i64, span));
    o.push(
        "mac",
        match inf.mac {
            Some(mac) => Value::string(mac.to_string(), span),
            None => Value::nothing(span),
        },
    );
    o.push(
        "ips",
        Value::list(
            inf.ips.into_iter().map(|ip| map_ip(ip, span)).collect(),
            span,
        ),
    );
    o.push("flags", flags);

    Value::record(o, span)
}

impl SimplePluginCommand for InterfacesCommand {
    type Plugin = crate::Plugin;

    fn name(&self) -> &str {
        "net"
    }

    fn usage(&self) -> &str {
        "Enumerate network interfaces on the current host"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name()).input_output_type(
            Type::Nothing,
            Type::Table(Box::new([
                ("name".to_string(), Type::String),
                ("description".to_string(), Type::String),
                ("if_index".to_string(), Type::Int),
                ("mac".to_string(), Type::String),
                (
                    "ips".to_string(),
                    Type::Table(Box::new([
                        ("type".to_string(), Type::String),
                        ("addr".to_string(), Type::String),
                        ("prefix".to_string(), Type::Int),
                    ])),
                ),
                (
                    "flags".to_string(),
                    Type::Record(Box::new([
                        ("is_up".to_string(), Type::Bool),
                        ("is_broadcast".to_string(), Type::Bool),
                        ("is_loopback".to_string(), Type::Bool),
                        ("is_point_to_point".to_string(), Type::Bool),
                        ("is_multicast".to_string(), Type::Bool),
                    ])),
                ),
            ])),
        )
    }

    fn run(
        &self,
        _plugin: &Self::Plugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let span = call.head;
        Ok(Value::list(
            pnet::datalink::interfaces()
                .into_iter()
                .map(|i| map_interface(i, span))
                .collect(),
            span,
        ))
    }
}
