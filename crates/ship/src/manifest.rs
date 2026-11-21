//! The update manifest (`latest.json`) and its detached signature
//! (`latest.json.sig`).
//!
//! The signature covers the manifest's exact bytes, so there is nothing to
//! canonicalise. Every artifact entry carries its SHA-256 and size, which makes
//! a verified manifest transitively authenticate the downloads it points to.

use crate::keys;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

/// Schema version written into every manifest.
pub const SCHEMA: u32 = 1;

/// One downloadable build.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// `linux`, `macos` or `windows` (values of `std::env::consts::OS`).
    pub os: String,
    /// `x86_64`, `aarch64`, ... (`std::env::consts::ARCH`).
    pub arch: String,
    /// `appimage`, `dmg`, `msi`, `msix`, `exe`, ...
    pub format: String,
    /// Absolute download URL.
    pub url: String,
    /// Lowercase hex SHA-256 of the file.
    pub sha256: String,
    /// Size in bytes.
    pub size: u64,
}

/// A release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Always [`SCHEMA`].
    pub schema: u32,
    /// App identifier; an updater refuses a manifest made for another app.
    pub app: String,
    /// Release version.
    pub version: String,
    /// Release notes (plain text), possibly empty.
    #[serde(default)]
    pub notes: String,
    /// Builds, one per OS, CPU and format.
    pub artifacts: Vec<Artifact>,
}

impl Manifest {
    /// Pretty JSON with a trailing newline.
    pub fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("manifest serialises");
        s.push('\n');
        s
    }

    /// Verify `sig_hex` over `bytes` with `public_hex`, then parse. The
    /// signature is checked first, so unauthenticated bytes are never parsed.
    pub fn parse_verified(bytes: &[u8], sig_hex: &str, public_hex: &str) -> Result<Self> {
        keys::verify(public_hex, bytes, sig_hex)?;
        let m: Manifest =
            serde_json::from_slice(bytes).map_err(|e| Error::BadManifest(e.to_string()))?;
        if m.schema != SCHEMA {
            return Err(Error::BadManifest(format!(
                "schema {} is not supported (expected {SCHEMA})",
                m.schema
            )));
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> Manifest {
        Manifest {
            schema: SCHEMA,
            app: "dev.example.demo".into(),
            version: "2.0.0".into(),
            notes: "Faster.".into(),
            artifacts: vec![Artifact {
                os: "linux".into(),
                arch: "x86_64".into(),
                format: "appimage".into(),
                url: "https://example.com/demo.AppImage".into(),
                sha256: "00".repeat(32),
                size: 4,
            }],
        }
    }

    #[test]
    fn verified_roundtrip_and_rejections() {
        let k = keys::generate().unwrap();
        let json = sample().to_json();
        let sig = keys::sign(&k.private_hex, json.as_bytes()).unwrap();
        assert_eq!(
            Manifest::parse_verified(json.as_bytes(), &sig, &k.public_hex).unwrap(),
            sample()
        );
        // One changed byte (a swapped URL) breaks the signature.
        let evil = json.replace("example.com", "evil.example");
        assert!(matches!(
            Manifest::parse_verified(evil.as_bytes(), &sig, &k.public_hex),
            Err(Error::BadSignature)
        ));
        // Signed by someone else.
        let other = keys::generate().unwrap();
        let sig2 = keys::sign(&other.private_hex, json.as_bytes()).unwrap();
        assert!(Manifest::parse_verified(json.as_bytes(), &sig2, &k.public_hex).is_err());
        // Correctly signed but wrong schema.
        let mut m = sample();
        m.schema = 99;
        let j = m.to_json();
        let s = keys::sign(&k.private_hex, j.as_bytes()).unwrap();
        assert!(matches!(
            Manifest::parse_verified(j.as_bytes(), &s, &k.public_hex),
            Err(Error::BadManifest(_))
        ));
    }
}
