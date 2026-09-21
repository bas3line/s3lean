//! The three encodings signing needs, written out rather than pulled in: URI
//! encoding by AWS's rules, lowercase hex, and standard base64.

/// URI-encodes by the rules the signature specification lays down: the
/// unreserved characters `A–Z a–z 0–9 - _ . ~` pass through, everything else
/// is `%XX` per UTF-8 byte with uppercase hex, and `/` is kept in a path but
/// encoded in a query value.
pub(crate) fn uri(input: &str, encode_slash: bool) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b'/' if !encode_slash => out.push('/'),
            _ => {
                out.push('%');
                out.push(HEX_UPPER[usize::from(byte >> 4)] as char);
                out.push(HEX_UPPER[usize::from(byte & 0xF)] as char);
            }
        }
    }
    out
}

/// A canonical query string: pairs sorted by name then value, each encoded.
pub(crate) fn query(pairs: &[(String, String)]) -> String {
    let mut encoded: Vec<(String, String)> = pairs
        .iter()
        .map(|(name, value)| (uri(name, true), uri(value, true)))
        .collect();
    encoded.sort();
    let mut out = String::new();
    for (index, (name, value)) in encoded.iter().enumerate() {
        if index > 0 {
            out.push('&');
        }
        out.push_str(name);
        out.push('=');
        out.push_str(value);
    }
    out
}

const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";
const HEX_LOWER: &[u8; 16] = b"0123456789abcdef";

pub(crate) fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX_LOWER[usize::from(byte >> 4)] as char);
        out.push(HEX_LOWER[usize::from(byte & 0xF)] as char);
    }
    out
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let word = (u32::from(chunk[0]) << 16)
            | chunk.get(1).map_or(0, |byte| u32::from(*byte) << 8)
            | chunk.get(2).map_or(0, |byte| u32::from(*byte));
        out.push(TABLE[((word >> 18) & 63) as usize] as char);
        out.push(TABLE[((word >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((word >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(word & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_encoding_follows_the_signing_rules() {
        assert_eq!(uri("test$file.text", false), "test%24file.text");
        assert_eq!(uri("a b/c~d", false), "a%20b/c~d");
        assert_eq!(uri("a/b", true), "a%2Fb");
        assert_eq!(uri("table=raw_otel_spans", false), "table%3Draw_otel_spans");
        assert_eq!(uri("é", false), "%C3%A9");
        assert_eq!(uri("", false), "");
    }

    #[test]
    fn query_strings_are_sorted_and_encoded() {
        let pairs = vec![
            ("b".to_owned(), "2".to_owned()),
            ("a".to_owned(), "x/y".to_owned()),
            ("a".to_owned(), "1".to_owned()),
        ];
        assert_eq!(query(&pairs), "a=1&a=x%2Fy&b=2");
        assert_eq!(query(&[]), "");
    }

    #[test]
    fn hex_is_lowercase() {
        assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
    }

    #[test]
    fn base64_matches_the_standard_alphabet_and_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
