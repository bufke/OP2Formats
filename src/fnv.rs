//! FNV-1a, 64-bit: the checksum of op2-rs's own files (saved games, network saves and replays) and the sim's state
//! hash. Not for anything security-related.

/// The FNV-1a 64-bit offset basis: the hash of nothing.
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

/// Continues hash `h` over `bytes`.
pub fn fnv1a(h: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(h, |h, &b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// The hash of `bytes`.
pub fn hash(bytes: &[u8]) -> u64 {
    fnv1a(FNV_OFFSET, bytes)
}

#[cfg(test)]
mod tests {
    #[test]
    fn known_values() {
        assert_eq!(super::hash(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(super::hash(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
