//! A content hash whose value means the same thing next year.
//!
//! This exists because the thing it replaces did not. The specification
//! fingerprint and the gate's output digest were both reached for
//! `std::collections::hash_map::DefaultHasher`, whose own documentation says:
//!
//! > The internal algorithm is not specified, and so it and its hashes should
//! > not be relied upon over releases.
//!
//! That is fine for a hash map, whose values never outlive the process, and
//! wrong for a value written into a record somebody reads later. The failure is
//! precise: a piece of work whose gates run either side of a toolchain upgrade
//! produces two different fingerprints for a file nobody touched, and the
//! certificate then reports a change that did not happen. A test comparing two
//! values computed by one binary cannot see it.
//!
//! So: FNV-1a, 64-bit. A specified algorithm with published constants, pinned
//! here by tests against vectors from outside this repository.
//!
//! **This detects change, not forgery.** An adversary who can rewrite these
//! records can rewrite the machine the agents run on, which is not a threat
//! model this product claims to defend against. That is also why the
//! certificate is unsigned: a signature needs a key, a key needs a trust root,
//! and a trust root needs an account. The certificate asks to be *checked*
//! rather than believed, so it carries the commands to check it instead.

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

/// The same, rendered the way every record here writes it.
pub fn hex(bytes: &[u8]) -> String {
    format!("{:016x}", of(bytes))
}

/// Accumulates over several pieces, for a value built from more than one thing.
///
/// Lengths are folded in between parts so that `["ab", "c"]` and `["a", "bc"]`
/// do not collide — a fingerprint over a folder of documents is exactly the
/// shape where that matters.
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

    /// Published FNV-1a 64 vectors. The point of testing against numbers from
    /// outside this repository is that the algorithm is pinned to something
    /// other than its own output — which is exactly what the hasher this
    /// replaces never had.
    #[test]
    fn matches_the_published_vectors() {
        assert_eq!(of(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(of(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(of(b"foobar"), 0x8594_4171_f739_67e8);
        assert_eq!(of(b"hello"), 0xa430_d846_80aa_bd0b);
    }

    /// FNV-1 and FNV-1a differ only in the order of the xor and the multiply,
    /// and they agree on the empty string. A vector set that stopped at `""`
    /// would pass against the wrong one of the two — which is not hypothetical:
    /// the first draft of this test carried a constant for the other variant
    /// and the implementation, which was right, is what failed.
    #[test]
    fn it_is_the_1a_variant_and_not_the_1_variant() {
        assert_ne!(of(b"a"), 0xaf63_bd4c_8601_b7be, "that is FNV-1, not FNV-1a");
        assert_ne!(of(b"foobar"), 0x340d_8765_a4dd_a9c2);
    }

    #[test]
    fn a_changed_byte_changes_the_value() {
        assert_ne!(of(b"hello"), of(b"hellp"));
    }

    /// The reason `Rolling` folds lengths in. Without it a fingerprint over a
    /// folder could be unchanged by moving a line from one file to the next.
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
