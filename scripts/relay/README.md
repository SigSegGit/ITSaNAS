# Self-hosted relay deployment (D5)

These files configure `iroh-relay` (from the `iroh-relay` crate — the same
relay server implementation `itsanas-net`'s M2 tests run in-process against,
not a custom one this project maintains) as the self-hosted relay on the
Freebox VM.

**This is an owner action.** Nothing in this repository's CI or test suite
deploys or reaches a real Freebox VM — there is no network path from a
Claude Code session to it. These files are the deployment artifact; running
them on the actual hardware is up to you, same as the CLA Assistant app
install was.

- `relay.example.toml` — config for the `iroh-relay` binary: TLS via
  Let's Encrypt, QUIC address discovery for hole-punching assistance, and
  a shared-token access control so the relay isn't an open proxy for
  arbitrary internet traffic.
- `itsanas-relay.service` — a systemd unit template to run it as a
  service.

## Steps

1. `cargo install iroh-relay --features server` on the Freebox VM (or
   cross-compile via `scripts/release.sh` and copy the binary over).
2. Point a domain's DNS at the VM's public IP; forward 443/tcp+udp.
3. Copy `relay.example.toml` to `/etc/itsanas/relay.toml`, set the real
   hostname, and put a real access token in `/etc/itsanas/relay.env` as
   `IROH_RELAY_ACCESS_TOKEN=...` (this overrides the config file's
   placeholder token entirely — never commit a real token to the config).
4. Install `itsanas-relay.service`, then
   `systemctl daemon-reload && systemctl enable --now itsanas-relay`.
5. Configure every `Node` to use it:
   ```rust
   RelayPolicy::SelfHosted {
       url: "https://relay.example.org".parse().unwrap(),
       auth_token: Some(token),
   }
   ```
   `RelayPolicy` has no variant that reaches iroh's public relay
   infrastructure (D4) — `SelfHosted` is the only one with a URL at all,
   and it's always this relay.
