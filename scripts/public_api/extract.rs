//! The public surface of a library crate, as its `lib.rs` declares it (P10):
//! the top-level `pub` declarations, one line each, in source order. Private
//! modules and `unreachable_pub` keep what is not re-exported out of the list,
//! so the list is the crate's API. Included by each crate's `tests/public_api.rs`
//! with `#[path]`: one definition for all of them.

/// The top-level `pub` declarations of `source`, one normalized line each. A
/// declaration runs to its `;`, its `{` (a body opens) or its `}` (a one-line
/// body); indented lines are not top-level.
pub fn surface(source: &str) -> String {
    let mut out = String::new();
    let mut open: Option<String> = None;
    for line in source.lines() {
        let item = match open.as_mut() {
            Some(item) => {
                item.push(' ');
                item.push_str(line.trim());
                item
            }
            None if line.starts_with("pub ") => open.insert(line.to_owned()),
            None => continue,
        };
        // A `use` group opens with `{` too: it runs to its `;`.
        let closes = item.ends_with(';')
            || item.ends_with('}')
            || (item.ends_with('{') && !item.contains("::{"));
        if closes {
            let done = item.strip_suffix(';').unwrap_or(item);
            let done = done.strip_suffix("{}").unwrap_or(done);
            let done = done.strip_suffix('{').unwrap_or(done);
            let normalized = done.split_whitespace().collect::<Vec<_>>().join(" ");
            out.push_str(&normalized);
            out.push('\n');
            open = None;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::surface;

    #[test]
    fn keeps_top_level_pub_declarations_and_drops_the_rest() {
        let src = "//! doc\nmod private;\npub mod open;\n    pub fn indented() {}\npub use a::{\n    B,\n    C,\n};\npub struct S {\n    pub field: u8,\n}\npub fn one() {}\npub(crate) fn hidden() {}\n";
        assert_eq!(
            surface(src),
            "pub mod open\npub use a::{ B, C, }\npub struct S\npub fn one()\n"
        );
    }
}
