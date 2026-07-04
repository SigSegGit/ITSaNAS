use iroh::{RelayConfig as IrohRelayConfig, RelayMap, RelayMode, RelayUrl};

/// How a [`crate::Node`] uses relays for NAT traversal (D4, D5).
///
/// There is deliberately no variant for iroh's public relay infrastructure
/// (`iroh::RelayMode::Default`/`Staging`) — D4 requires this project to
/// never fall back to it, so the type itself makes that infrastructure
/// unreachable rather than relying on callers to avoid picking it.
#[derive(Debug, Clone)]
pub enum RelayPolicy {
    /// No relay: only direct connections (M1's LAN-only behavior).
    Disabled,
    /// Use exactly one self-hosted relay (D5 — e.g. the Freebox VM), never
    /// iroh's public relay infrastructure.
    SelfHosted {
        url: RelayUrl,
        /// Optional shared bearer token, if the relay is configured with
        /// `access.shared_token` (see `scripts/relay/relay.example.toml`).
        auth_token: Option<String>,
    },
}

impl RelayPolicy {
    pub(crate) fn into_relay_mode(self) -> RelayMode {
        match self {
            RelayPolicy::Disabled => RelayMode::Disabled,
            RelayPolicy::SelfHosted { url, auth_token } => {
                let mut config = IrohRelayConfig::from(url);
                if let Some(token) = auth_token {
                    config = config.with_auth_token(token);
                }
                RelayMode::Custom(RelayMap::from(config))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_maps_to_relay_mode_disabled() {
        assert!(matches!(
            RelayPolicy::Disabled.into_relay_mode(),
            RelayMode::Disabled
        ));
    }

    #[test]
    fn self_hosted_maps_to_relay_mode_custom() {
        let url: RelayUrl = "https://relay.example.org".parse().unwrap();
        let mode = RelayPolicy::SelfHosted {
            url,
            auth_token: Some("secret".to_string()),
        }
        .into_relay_mode();
        assert!(matches!(mode, RelayMode::Custom(_)));
    }
}
