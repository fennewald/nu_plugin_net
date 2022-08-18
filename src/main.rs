extern crate pnet;
use nu_plugin::{self, EvaluatedCall, LabeledError};
use nu_protocol::{Category, Signature, Value};
use pnet::datalink::{self, MacAddr, NetworkInterface};
use pnet::ipnetwork::IpNetwork;

pub struct Plugin;

fn flags_to_nu(call: &EvaluatedCall, interface: &NetworkInterface) -> Value {
    Value::Record {
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
                span: call.head,
            },
            Value::Bool {
                val: interface.is_broadcast(),
                span: call.head,
            },
            Value::Bool {
                val: interface.is_loopback(),
                span: call.head,
            },
            Value::Bool {
                val: interface.is_point_to_point(),
                span: call.head,
            },
            Value::Bool {
                val: interface.is_multicast(),
                span: call.head,
            },
        ],
        span: call.head,
    }
}

fn mac_to_nu(call: &EvaluatedCall, mac: Option<MacAddr>) -> Value {
    if let Some(mac) = mac {
        Value::String {
            val: format!("{}", mac),
            span: call.head,
        }
    } else {
        Value::Nothing { span: call.head }
    }
}

fn ip_to_nu(call: &EvaluatedCall, ip: &IpNetwork) -> Value {
    let type_name = match ip {
        IpNetwork::V4(..) => "v4",
        IpNetwork::V6(..) => "v6",
    };
    Value::Record {
        cols: vec!["type".to_string(), "addr".to_string(), "prefix".to_string()],
        vals: vec![
            Value::String {
                val: type_name.to_string(),
                span: call.head,
            },
            Value::String {
                val: format!("{}", ip),
                span: call.head,
            },
            Value::Int {
                val: ip.prefix() as i64,
                span: call.head,
            },
        ],
        span: call.head,
    }
}

/// Convert a slice of ipnetworks to nushell values
fn ips_to_nu(call: &EvaluatedCall, ips: &[IpNetwork]) -> Value {
    Value::List {
        vals: ips.iter().map(|ip| ip_to_nu(call, ip)).collect(),
        span: call.head,
    }
}

impl nu_plugin::Plugin for Plugin {
    fn signature(&self) -> Vec<Signature> {
        vec![Signature::build("net")
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
                    cols: cols.clone(),
                    vals: vec![
                        Value::String {
                            val: interface.name.clone(),
                            span: call.head,
                        },
                        Value::String {
                            val: interface.description.clone(),
                            span: call.head,
                        },
                        Value::Int {
                            val: interface.index as i64,
                            span: call.head,
                        },
                        mac_to_nu(call, interface.mac),
                        ips_to_nu(call, &interface.ips),
                        flags_to_nu(call, interface),
                    ],
                    span: call.head,
                })
                .collect(),
            span: call.head,
        })
    }
}

fn main() {
    nu_plugin::serve_plugin(&mut Plugin {}, nu_plugin::CapnpSerializer {})
}
