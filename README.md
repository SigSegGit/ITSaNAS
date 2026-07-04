# ITSaNAS — "it's a NAS"

A decentralized, end-to-end-encrypted, redundant storage network for a group of
trusted friends. Each participant contributes local disk space to a shared P2P
pool and gets multi-location, versioned, encrypted storage in return — like a
self-hosted Dropbox, owned by the group, with no dependency on a big cloud
provider.

Every user keeps a full local copy of their own data: the network provides
off-site redundancy and multi-device access, not primary storage. Day-to-day
file access is local-first.

## Status

Early prototype (M0: core crypto + chunking library). See
[`STATUS.md`](STATUS.md) for the current milestone and next steps, and
[`ARCHITECTURE.md`](ARCHITECTURE.md) for the design.

## Open code, private networks

This repository is public and the code is free software (AGPL-3.0), but
**joining a running ITSaNAS network is invite-only**. Anyone can read, audit,
fork, or self-host this code; nobody can join *your* group's network without
being invited into it by an existing member.

## License

AGPL-3.0 (see [`LICENSE`](LICENSE)). External contributions require a signed
Contributor License Agreement — see [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Milestones

| Milestone | Goal |
|---|---|
| M0 | Crypto + chunking library, unit tested |
| M1 | Two-node LAN store/retrieve |
| M2 | NAT traversal via a self-hosted relay |
| M3 | Mirroring, repair, and scrubbing for hostile/unreliable storage backends |
| M4 | Accounts and quotas |
| M5 | Android client |
| M6 | Acceptance run on real hardware (two real users) |
| M7 | Erasure coding once the network reaches 4+ nodes |
