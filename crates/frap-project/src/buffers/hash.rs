// SPDX-License-Identifier: GPL-3.0-or-later
//! Content hashes: the first 16 bytes (32 lowercase hex characters) of the SHA-256 of an
//! entry's bytes (§15). Used for `buffers/<hash>.bin` and `images/<hash>.<ext>`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

/// SHA-256 truncated to 128 bits, written as 32 lowercase hex characters.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash([u8; ContentHash::BYTES]);

impl ContentHash {
    /// Number of digest bytes kept.
    pub const BYTES: usize = 16;
    /// Length of the hex spelling (`buffers/<32 hex>.bin`).
    pub const HEX_LEN: usize = 2 * Self::BYTES;

    /// Hash of `bytes`.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut out = [0u8; Self::BYTES];
        out.copy_from_slice(&digest[..Self::BYTES]);
        Self(out)
    }

    /// The raw truncated digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; Self::BYTES] {
        &self.0
    }

    /// Lowercase hex spelling (32 characters).
    #[must_use]
    pub fn to_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::with_capacity(Self::HEX_LEN);
        for b in self.0 {
            s.push(char::from(HEX[usize::from(b >> 4)]));
            s.push(char::from(HEX[usize::from(b & 0x0f)]));
        }
        s
    }
}

/// A string that is not a 32-character lowercase hex content hash.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid content hash {0:?} (expected 32 lowercase hex characters)")]
pub struct ParseHashError(pub String);

impl FromStr for ContentHash {
    type Err = ParseHashError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn nibble(c: u8) -> Option<u8> {
            match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                _ => None,
            }
        }
        let err = || ParseHashError(s.to_owned());
        let bytes = s.as_bytes();
        if bytes.len() != Self::HEX_LEN {
            return Err(err());
        }
        let mut out = [0u8; Self::BYTES];
        for (i, pair) in bytes.chunks_exact(2).enumerate() {
            out[i] = (nibble(pair[0]).ok_or_else(err)? << 4) | nibble(pair[1]).ok_or_else(err)?;
        }
        Ok(Self(out))
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({self})")
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_prefix_of_known_input() {
        // sha256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        assert_eq!(
            ContentHash::of(b"abc").to_hex(),
            "ba7816bf8f01cfea414140de5dae2223"
        );
        // sha256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        assert_eq!(
            ContentHash::of(b"").to_string(),
            "e3b0c44298fc1c149afbf4c8996fb924"
        );
    }

    #[test]
    fn parse_round_trip_and_rejects() {
        let h = ContentHash::of(b"frap");
        assert_eq!(h.to_hex().parse::<ContentHash>(), Ok(h));
        assert!(
            "BA7816BF8F01CFEA414140DE5DAE2223"
                .parse::<ContentHash>()
                .is_err()
        );
        assert!("ba7816bf8f01cfea".parse::<ContentHash>().is_err());
        assert!(
            "ba7816bf8f01cfea414140de5dae222g"
                .parse::<ContentHash>()
                .is_err()
        );
        assert!(
            "ba7816bf8f01cfea414140de5dae2223ff"
                .parse::<ContentHash>()
                .is_err()
        );
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(serde_json::from_str::<ContentHash>(&json).unwrap(), h);
    }
}
