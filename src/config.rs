// SPDX-License-Identifier: MIT

use std::net::IpAddr;

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};

#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 3]
pub struct Config {
    bridge_ip: Option<IpAddr>,
    username: Option<String>,
}

impl Config {
    pub fn get_bridge_ip(&self) -> Option<&IpAddr> {
        self.bridge_ip.as_ref()
    }

    pub fn get_username(&self) -> Option<&str> {
        self.username.as_deref()
    }
}
