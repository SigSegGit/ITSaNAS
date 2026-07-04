use rand_core::OsRng;

/// An Ed25519 keypair used for peer identity and signing (D10).
pub struct SigningIdentity {
    pub signing_key: ed25519_dalek::SigningKey,
    pub verifying_key: ed25519_dalek::VerifyingKey,
}

/// Generates a new random Ed25519 signing identity.
pub fn generate_signing_identity() -> SigningIdentity {
    let signing_key = ed25519_dalek::SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    SigningIdentity {
        signing_key,
        verifying_key,
    }
}

/// An X25519 keypair used for Diffie-Hellman key exchange (D10).
pub struct ExchangeIdentity {
    pub secret: x25519_dalek::StaticSecret,
    pub public: x25519_dalek::PublicKey,
}

/// Generates a new random X25519 key-exchange identity.
pub fn generate_exchange_identity() -> ExchangeIdentity {
    let secret = x25519_dalek::StaticSecret::random_from_rng(OsRng);
    let public = x25519_dalek::PublicKey::from(&secret);
    ExchangeIdentity { secret, public }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Signer;
    use ed25519_dalek::Verifier;

    #[test]
    fn signing_identity_signs_and_verifies() {
        let identity = generate_signing_identity();
        let message = b"a message from this peer";
        let signature = identity.signing_key.sign(message);
        assert!(identity.verifying_key.verify(message, &signature).is_ok());
    }

    #[test]
    fn signing_identity_rejects_wrong_message() {
        let identity = generate_signing_identity();
        let signature = identity.signing_key.sign(b"original message");
        assert!(identity
            .verifying_key
            .verify(b"tampered message", &signature)
            .is_err());
    }

    #[test]
    fn two_signing_identities_are_distinct() {
        let a = generate_signing_identity();
        let b = generate_signing_identity();
        assert_ne!(a.verifying_key.to_bytes(), b.verifying_key.to_bytes());
    }

    #[test]
    fn exchange_identities_agree_on_a_shared_secret() {
        let alice = generate_exchange_identity();
        let bob = generate_exchange_identity();

        let alice_shared = alice.secret.diffie_hellman(&bob.public);
        let bob_shared = bob.secret.diffie_hellman(&alice.public);

        assert_eq!(alice_shared.as_bytes(), bob_shared.as_bytes());
    }

    #[test]
    fn two_exchange_identities_are_distinct() {
        let a = generate_exchange_identity();
        let b = generate_exchange_identity();
        assert_ne!(a.public.as_bytes(), b.public.as_bytes());
    }
}
