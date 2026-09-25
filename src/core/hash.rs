//! A content hash stable across releases: FNV-1a 64-bit, pinned by published
//! test vectors. `DefaultHasher` is unspecified across Rust releases, so a
//! recorded fingerprint would change on a toolchain upgrade. This detects
//! change, not forgery; the certificate carries commands to check it instead
//! of a signature.

const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over bytes.
pub fn of(bytes: &[u8]) -> u64 {
    let mut h = OFFSET;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// [`of`] as 16 hex digits, as every record writes it.
pub fn hex(bytes: &[u8]) -> String {
    format!("{:016x}", of(bytes))
}

/// [`of`] fed in pieces, for a stream too long to keep (a gate's full output).
#[derive(Debug, Clone)]
pub struct Stream(u64);

impl Default for Stream {
    fn default() -> Self {
        Self(OFFSET)
    }
}

impl Stream {
    pub fn update(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    pub fn hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

/// A hash over several parts, with lengths folded in so `["ab", "c"]` and
/// `["a", "bc"]` do not collide.
#[derive(Debug, Clone)]
pub struct Rolling(u64);

impl Default for Rolling {
    fn default() -> Self {
        Self(OFFSET)
    }
}

impl Rolling {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, bytes: &[u8]) {
        for b in (bytes.len() as u64).to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(PRIME);
        }
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(PRIME);
        }
    }

    pub fn push_str(&mut self, s: &str) {
        self.push(s.as_bytes());
    }

    pub fn hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_in_pieces_is_the_whole() {
        let mut s = Stream::default();
        s.update(b"foo");
        s.update(b"");
        s.update(b"bar");
        assert_eq!(s.hex(), hex(b"foobar"));
    }

    #[test]
    fn matches_the_published_vectors() {
        assert_eq!(of(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(of(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(of(b"foobar"), 0x8594_4171_f739_67e8);
        assert_eq!(of(b"hello"), 0xa430_d846_80aa_bd0b);
    }

    /// FNV-1 and FNV-1a agree on `""`, so a non-empty vector tells them apart.
    #[test]
    fn it_is_the_1a_variant_and_not_the_1_variant() {
        assert_ne!(of(b"a"), 0xaf63_bd4c_8601_b7be, "that is FNV-1, not FNV-1a");
        assert_ne!(of(b"foobar"), 0x340d_8765_a4dd_a9c2);
    }

    #[test]
    fn a_changed_byte_changes_the_value() {
        assert_ne!(of(b"hello"), of(b"hellp"));
    }

    #[test]
    fn parts_cannot_be_reassociated() {
        let mut a = Rolling::new();
        a.push_str("ab");
        a.push_str("c");
        let mut b = Rolling::new();
        b.push_str("a");
        b.push_str("bc");
        assert_ne!(a.hex(), b.hex());
    }

    #[test]
    fn rolling_is_deterministic() {
        let mut a = Rolling::new();
        a.push_str("one");
        a.push_str("two");
        let mut b = Rolling::new();
        b.push_str("one");
        b.push_str("two");
        assert_eq!(a.hex(), b.hex());
    }
}
