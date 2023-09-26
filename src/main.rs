extern crate pnet;
use nu_plugin::{self, EvaluatedCall, LabeledError};
use nu_protocol::{Category, PluginSignature, Record, Value};
use pnet::datalink::{self, MacAddr, NetworkInterface};
use pnet::ipnetwork::IpNetwork;

pub struct Plugin;

fn flags_to_nu(call: &EvaluatedCall, interface: &NetworkInterface) -> Value {
    Value::Record {
        val: Record {
            cols: vec![
                "is_up".to_string(),
                "is_broadcast".to_string(),
                "is_loopback".to_string(),
                "is_point_to_point".to_string(),
                "is_multicast".to_string(),
            ],
            vals: vec![
                Value::Bool {
                    val: interface.is_up(),
                    internal_span: call.head,
                },
                Value::Bool {
                    val: interface.is_broadcast(),
                    internal_span: call.head,
                },
                Value::Bool {
                    val: interface.is_loopback(),
                    internal_span: call.head,
                },
                Value::Bool {
                    val: interface.is_point_to_point(),
                    internal_span: call.head,
                },
                Value::Bool {
                    val: interface.is_multicast(),
                    internal_span: call.head,
                },
            ],
        },
        internal_span: call.head,
    }
}

fn mac_to_nu(call: &EvaluatedCall, mac: Option<MacAddr>) -> Value {
    if let Some(mac) = mac {
        Value::String {
            val: format!("{}", mac),
            internal_span: call.head,
        }
    } else {
        Value::Nothing {
            internal_span: call.head,
        }
    }
}

fn ip_to_nu(call: &EvaluatedCall, ip: &IpNetwork) -> Value {
    let type_name = match ip {
        IpNetwork::V4(..) => "v4",
        IpNetwork::V6(..) => "v6",
    };
    Value::Record {
        val: Record {
            cols: vec!["type".to_string(), "addr".to_string(), "prefix".to_string()],
            vals: vec![
                Value::String {
                    val: type_name.to_string(),
                    internal_span: call.head,
                },
                Value::String {
                    val: format!("{}", ip),
                    internal_span: call.head,
                },
                Value::Int {
                    val: ip.prefix() as i64,
                    internal_span: call.head,
                },
            ],
        },
        internal_span: call.head,
    }
}

/// Convert a slice of ipnetworks to nushell values
fn ips_to_nu(call: &EvaluatedCall, ips: &[IpNetwork]) -> Value {
    Value::List {
        vals: ips.iter().map(|ip| ip_to_nu(call, ip)).collect(),
        internal_span: call.head,
    }
}

impl nu_plugin::Plugin for Plugin {
    fn signature(&self) -> Vec<PluginSignature> {
        vec![PluginSignature::build("net")
            .usage("List network interfaces")
            .category(Category::Experimental)]
    }

    fn run(
        &mut self,
        _name: &str,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let cols = vec![
            "name".to_string(),
            "description".to_string(),
            "if_index".to_string(),
            "mac".to_string(),
            "ips".to_string(),
            "flags".to_string(),
        ];
        Ok(Value::List {
            vals: datalink::interfaces()
                .iter_mut()
                .map(|interface| Value::Record {
                    val: Record {
                        cols: cols.clone(),
                        vals: vec![
                            Value::String {
                                val: interface.name.clone(),
                                internal_span: call.head,
                            },
                            Value::String {
                                val: interface.description.clone(),
                                internal_span: call.head,
                            },
                            Value::Int {
                                val: interface.index as i64,
                                internal_span: call.head,
                            },
                            mac_to_nu(call, interface.mac),
                            ips_to_nu(call, &interface.ips),
                            flags_to_nu(call, interface),
                        ],
                    },
                    internal_span: call.head,
                })
                .collect(),
            internal_span: call.head,
        })
    }
}

fn main() {
    nu_plugin::serve_plugin(&mut Plugin {}, nu_plugin::JsonSerializer {})
}
