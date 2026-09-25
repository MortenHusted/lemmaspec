//! Shared destination policy for source links in human reports.

/// Whether a source may be linked: a repository-relative path, a fragment,
/// or an HTTPS URL. Renderers must still escape the destination for their
/// output format. Rejected destinations can be displayed as plain text.
pub fn is_safe_source(source: &str) -> bool {
    if source.is_empty()
        || source
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || ch == '\\')
    {
        return false;
    }

    // Check encoded separators and controls too, so encoded text cannot
    // disguise an absolute path or a parent-directory component.
    let mut decoded = Vec::with_capacity(source.len());
    let mut bytes = source.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let Some(high) = bytes.next().and_then(|byte| (byte as char).to_digit(16)) else {
                return false;
            };
            let Some(low) = bytes.next().and_then(|byte| (byte as char).to_digit(16)) else {
                return false;
            };
            let byte = (high * 16 + low) as u8;
            if byte.is_ascii_control() || byte == b'\\' {
                return false;
            }
            decoded.push(byte);
        } else {
            decoded.push(byte);
        }
    }
    let Ok(decoded) = std::str::from_utf8(&decoded) else {
        return false;
    };
    if decoded.chars().any(char::is_control) {
        return false;
    }
    if source
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
    {
        let authority = source[8..]
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default();
        return !authority.is_empty();
    }
    if source.starts_with('#') {
        return true;
    }
    let path = decoded.split(['?', '#']).next().unwrap_or_default();
    !path.is_empty()
        && !path.starts_with('/')
        && !path.split('/').any(|part| part == "..")
        && !path.split('/').next().unwrap_or_default().contains(':')
}
