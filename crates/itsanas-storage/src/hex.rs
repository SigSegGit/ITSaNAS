/// Decodes a lowercase hex string (as produced by `ChunkId`'s `Display`)
/// back into raw bytes, e.g. when reconstructing a `ChunkId` from a shard
/// filename. Returns `None` for anything that isn't valid lowercase hex of
/// an even length.
pub fn decode(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_valid_hex() {
        assert_eq!(decode("00ff10"), Some(vec![0x00, 0xff, 0x10]));
    }

    #[test]
    fn rejects_odd_length() {
        assert_eq!(decode("abc"), None);
    }

    #[test]
    fn rejects_non_hex_characters() {
        assert_eq!(decode("zz"), None);
    }
}
