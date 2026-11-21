//! Ed25519 keys as lowercase hex, and signing/verifying raw bytes.
//!
//! The private key is a 32-byte seed in hex (64 characters); the public key
//! is 32 bytes (64 characters). Keep the seed out of the repository: pass it
//! to `alloy bundle` through `ALLOY_UPDATE_KEY`; only the public key goes into
//! the app.

use crate::{Error, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// A freshly generated key pair, both halves in hex.
pub struct KeyPair {
    /// 32-byte seed, hex. Secret.
    pub private_hex: String,
    /// 32-byte public key, hex. Embed in the app.
    pub public_hex: String,
}

/// Generate a key pair from the operating system's random source.
pub fn generate() -> Result<KeyPair> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| Error::BadKey(format!("no OS randomness: {e}")))?;
    Ok(KeyPair {
        private_hex: hex(&seed),
        public_hex: hex(SigningKey::from_bytes(&seed).verifying_key().as_bytes()),
    })
}

/// Public key (hex) belonging to a private seed (hex).
pub fn public_of(private_hex: &str) -> Result<String> {
    let k = signing_key(private_hex)?;
    Ok(hex(k.verifying_key().as_bytes()))
}

/// Sign `data`, returning the 64-byte signature as hex.
pub fn sign(private_hex: &str, data: &[u8]) -> Result<String> {
    Ok(hex(&signing_key(private_hex)?.sign(data).to_bytes()))
}

/// Check `signature_hex` over `data` against `public_hex`.
pub fn verify(public_hex: &str, data: &[u8], signature_hex: &str) -> Result<()> {
    let pk: [u8; 32] = unhex(public_hex)?
        .try_into()
        .map_err(|_| Error::BadKey("public key must be 32 bytes".into()))?;
    let pk = VerifyingKey::from_bytes(&pk).map_err(|e| Error::BadKey(e.to_string()))?;
    let sig: [u8; 64] = unhex(signature_hex.trim())?
        .try_into()
        .map_err(|_| Error::BadKey("signature must be 64 bytes".into()))?;
    pk.verify(data, &Signature::from_bytes(&sig))
        .map_err(|_| Error::BadSignature)
}

fn signing_key(private_hex: &str) -> Result<SigningKey> {
    let seed: [u8; 32] = unhex(private_hex.trim())?
        .try_into()
        .map_err(|_| Error::BadKey("private key must be 32 bytes".into()))?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Lowercase hex.
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|v| format!("{v:02x}")).collect()
}

fn unhex(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return Err(Error::BadKey(
            "expected an even number of hex digits".into(),
        ));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| Error::BadKey("not hex".into())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_roundtrip_and_tamper() {
        let k = generate().unwrap();
        assert_eq!(public_of(&k.private_hex).unwrap(), k.public_hex);
        let sig = sign(&k.private_hex, b"hello").unwrap();
        verify(&k.public_hex, b"hello", &sig).unwrap();
        assert!(matches!(
            verify(&k.public_hex, b"hellO", &sig),
            Err(Error::BadSignature)
        ));
        let other = generate().unwrap();
        assert!(verify(&other.public_hex, b"hello", &sig).is_err());
        assert!(verify(&k.public_hex, b"hello", "abcd").is_err());
        assert!(verify("zz", b"x", &sig).is_err());
    }

    #[test]
    fn rfc8032_test_vector_1() {
        // RFC 8032 section 7.1, TEST 1 (empty message): proves we are
        // compatible with every other Ed25519 implementation.
        let sk = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
        let pk = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        let sig = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";
        assert_eq!(public_of(sk).unwrap(), pk);
        assert_eq!(sign(sk, b"").unwrap(), sig);
        verify(pk, b"", sig).unwrap();
    }
}
