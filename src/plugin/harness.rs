use std::cell::RefCell;

use nu_plugin_protocol::{PluginInput, PluginOutput};
use nu_protocol::ShellError;

use crate::channel::{Receiver, Sender};

use super::Result;
