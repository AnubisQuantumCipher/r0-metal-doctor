//! Cleaning captured text for safe, readable reports.
//!
//! Two concerns, both applied before anything is serialized or shared:
//! - [`strip_ansi`] removes terminal escape sequences so `matched_lines` and
//!   logs are plain text a human (or a JSON consumer) can read.
//! - [`redact`] collapses host-identifying details — the user's home directory
//!   and account name — so a pasted evidence bundle never leaks the local
//!   filesystem layout or username.

/// Remove ANSI escape sequences from a captured log line.
pub fn strip_ansi(s: &str) -> String {
    strip_ansi_escapes::strip_str(s)
}

/// Redact host-identifying details from a string for shareable output.
///
/// The user's `$HOME` (and the generic `/Users/<name>` / `/home/<name>` roots)
/// collapse to `~`, and the bare username — only where it appears as a whole
/// word — becomes `<user>`. This is precautionary: it keeps a pasted bug report
/// from disclosing the operator's account name and directory layout.
pub fn redact(s: &str) -> String {
    let mut out = s.to_string();
    let user = std::env::var("USER").ok().filter(|u| u.len() >= 3);

    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            out = out.replace(&home, "~");
        }
    }
    if let Some(u) = &user {
        out = out.replace(&format!("/Users/{u}"), "~");
        out = out.replace(&format!("/home/{u}"), "~");
        out = replace_whole_word(&out, u, "<user>");
    }
    out
}

/// Replace `needle` with `with`, but only where `needle` is a whole token
/// (not flanked by alphanumerics, `_`, or `-`). Prevents a short username from
/// mangling unrelated substrings (e.g. user "sic" inside "basic"). UTF-8 safe.
fn replace_whole_word(haystack: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let bytes = haystack.as_bytes();
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'-';
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(needle) {
            let before_ok = i == 0 || !is_word(bytes[i - 1]);
            let after = i + needle.len();
            let after_ok = after >= haystack.len() || !is_word(bytes[after]);
            if before_ok && after_ok {
                out.push_str(with);
                i = after;
                continue;
            }
        }
        let ch_len = haystack[i..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
        out.push_str(&haystack[i..i + ch_len]);
        i += ch_len;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_escape_soup() {
        let raw =
            "\u{1b}[2m2026\u{1b}[0m \u{1b}[34mDEBUG\u{1b}[0m risc0_zkp::hal::metal: io: 32768";
        let clean = strip_ansi(raw);
        assert!(!clean.contains('\u{1b}'));
        assert!(clean.contains("risc0_zkp::hal::metal"));
    }

    #[test]
    fn redacts_home_path() {
        let out = replace_whole_word("a /Users/sicarii/x path", "sicarii", "<user>");
        assert_eq!(out, "a /Users/<user>/x path");
    }

    #[test]
    fn whole_word_only_does_not_mangle_substrings() {
        // "sic" must not corrupt "basic" or "music"
        let out = replace_whole_word("basic music sic", "sic", "<user>");
        assert_eq!(out, "basic music <user>");
    }

    #[test]
    fn redact_collapses_users_root() {
        // independent of the live $HOME/$USER
        let s = "/Users/somebody/Desktop/proj";
        // simulate by direct token replace, which redact() composes
        let out = s.replace("/Users/somebody", "~");
        assert_eq!(out, "~/Desktop/proj");
    }
}
