//! Short random identifiers: 40 random bits as 8 lowercase Crockford base32 characters.
//! Passage IDs are the bare code (it is also the marker in the Markdown); other kinds carry a prefix.

const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

pub const BOOK: &str = "bk";
pub const CHAPTER: &str = "ch";
pub const SCENE: &str = "sc";
pub const ENTITY: &str = "en";
pub const RELATIONSHIP: &str = "rl";
pub const EVENT: &str = "ev";
pub const FACT: &str = "ft";

/// A fresh 8-character code. Randomness comes from the operating system.
pub fn code() -> String {
    let mut bytes = [0u8; 5];
    getrandom::fill(&mut bytes).expect("operating system random source");
    let mut n = bytes.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
    let mut out = [0u8; 8];
    for slot in out.iter_mut().rev() {
        *slot = ALPHABET[(n & 31) as usize];
        n >>= 5;
    }
    String::from_utf8(out.to_vec()).expect("ascii alphabet")
}

pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", code())
}

/// True for a bare passage code.
pub fn is_code(s: &str) -> bool {
    s.len() == 8 && s.bytes().all(|b| ALPHABET.contains(&b))
}

/// True for `<prefix>_<code>`. Checked before an ID taken from a URL is used anywhere near a path.
pub fn is_id(s: &str, prefix: &str) -> bool {
    s.strip_prefix(prefix)
        .and_then(|rest| rest.strip_prefix('_'))
        .is_some_and(is_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_have_the_right_shape_and_differ() {
        let a = code();
        let b = code();
        assert!(is_code(&a) && is_code(&b));
        assert_ne!(a, b);
    }

    #[test]
    fn ids_carry_their_prefix() {
        let id = new_id(SCENE);
        assert!(id.starts_with("sc_") && is_id(&id, SCENE));
        assert!(!is_id(&id, BOOK));
    }

    #[test]
    fn rejects_malformed_ids() {
        for bad in [
            "",
            "sc_",
            "sc_ABCDEFGH",
            "sc_abc",
            "sc_abcdefghi",
            "../etc",
            "sc_0123456u",
            "sc-01234567",
        ] {
            assert!(!is_id(bad, SCENE), "{bad}");
        }
        assert!(!is_code("0123456i")); // i is not in the alphabet used here
    }
}
