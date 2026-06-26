/// True when the first bytes look like an HTTP request line.
pub fn is_http_post(prefix: &[u8]) -> bool {
    prefix.starts_with(b"POST ")
}

/// Parse `Content-Length` from an HTTP header block (bytes before the body).
pub fn content_length_from_headers(headers: &[u8]) -> Option<usize> {
    let text = std::str::from_utf8(headers).ok()?;
    for line in text.split("\r\n") {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.eq_ignore_ascii_case("content-length") {
            return value.trim().parse().ok();
        }
    }
    None
}

/// Index after the header terminator `\r\n\r\n`, if present in `buf`.
pub fn header_body_split(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_http_post_prefix() {
        assert!(is_http_post(b"POST /snapshot HTTP/1.1\r\n"));
        assert!(!is_http_post(b"{\"tab\":\"0\"}"));
    }

    #[test]
    fn parses_content_length_case_insensitive() {
        let headers = b"POST /snapshot HTTP/1.1\r\nHost: localhost\r\nContent-Length: 42\r\n";
        assert_eq!(content_length_from_headers(headers), Some(42));
    }

    #[test]
    fn header_body_split_finds_blank_line() {
        let buf = b"POST /x HTTP/1.1\r\nContent-Length: 0\r\n\r\n";
        assert_eq!(header_body_split(buf), Some(buf.len()));
    }
}
