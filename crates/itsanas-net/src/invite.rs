use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use iroh::EndpointId;

use crate::error::NetError;

const INVITE_CONTEXT: &[u8] = b"itsanas-invite-v1";

/// A signed invitation to join this network (D12).
///
/// Commits to a bootstrap peer's identity and an expiry, signed by an
/// existing member. This is how a new node learns *who to trust* as its
/// first contact — it deliberately does not carry that peer's current
/// network address, since addresses are looked up fresh at connect time
/// (peer discovery) rather than baked into a long-lived token that would
/// go stale.
///
/// The code is public (D12/D11); joining a running network is not — an
/// `Invite` is the credential that makes that distinction real.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub bootstrap_id: EndpointId,
    pub expires_at_unix: u64,
    pub issuer: VerifyingKey,
    signature: Signature,
}

impl Invite {
    /// Issues a new invite, signed by `issuer`.
    pub fn issue(issuer: &SigningKey, bootstrap_id: EndpointId, expires_at_unix: u64) -> Self {
        let message = signed_message(&bootstrap_id, expires_at_unix);
        let signature = issuer.sign(&message);
        Invite {
            bootstrap_id,
            expires_at_unix,
            issuer: issuer.verifying_key(),
            signature,
        }
    }

    /// Verifies the invite's signature and that it has not expired as of
    /// `now_unix`.
    ///
    /// This only proves the invite is well-formed and unexpired — it does
    /// not check whether `self.issuer` is actually a trusted member of
    /// this network. That's a membership-list lookup the caller makes
    /// (accounts/membership land in M4); this type only handles the
    /// cryptographic half.
    pub fn verify(&self, now_unix: u64) -> Result<(), NetError> {
        if now_unix > self.expires_at_unix {
            return Err(NetError::Protocol("invite has expired".to_string()));
        }
        let message = signed_message(&self.bootstrap_id, self.expires_at_unix);
        self.issuer
            .verify(&message, &self.signature)
            .map_err(|_| NetError::Protocol("invite signature is invalid".to_string()))
    }
}

fn signed_message(bootstrap_id: &EndpointId, expires_at_unix: u64) -> Vec<u8> {
    let mut message = Vec::with_capacity(INVITE_CONTEXT.len() + 32 + 8);
    message.extend_from_slice(INVITE_CONTEXT);
    message.extend_from_slice(bootstrap_id.as_bytes());
    message.extend_from_slice(&expires_at_unix.to_be_bytes());
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signing_key() -> SigningKey {
        itsanas_crypto::identity::generate_signing_identity().signing_key
    }

    fn bootstrap_id() -> EndpointId {
        let key = signing_key();
        EndpointId::from_bytes(key.verifying_key().as_bytes()).expect("valid key")
    }

    #[test]
    fn valid_unexpired_invite_verifies() {
        let issuer = signing_key();
        let invite = Invite::issue(&issuer, bootstrap_id(), 1_000);
        assert!(invite.verify(500).is_ok());
        assert!(invite.verify(1_000).is_ok());
    }

    #[test]
    fn expired_invite_fails_verification() {
        let issuer = signing_key();
        let invite = Invite::issue(&issuer, bootstrap_id(), 1_000);
        assert!(invite.verify(1_001).is_err());
    }

    #[test]
    fn tampered_bootstrap_id_fails_verification() {
        let issuer = signing_key();
        let mut invite = Invite::issue(&issuer, bootstrap_id(), 1_000);
        invite.bootstrap_id = bootstrap_id();
        assert!(invite.verify(500).is_err());
    }

    #[test]
    fn tampered_expiry_fails_verification() {
        let issuer = signing_key();
        let mut invite = Invite::issue(&issuer, bootstrap_id(), 1_000);
        invite.expires_at_unix = 2_000;
        assert!(invite.verify(500).is_err());
    }

    #[test]
    fn invite_signed_by_a_different_issuer_key_does_not_verify() {
        let issuer = signing_key();
        let mut invite = Invite::issue(&issuer, bootstrap_id(), 1_000);
        let other_issuer = signing_key();
        invite.issuer = other_issuer.verifying_key();
        assert!(invite.verify(500).is_err());
    }
}
