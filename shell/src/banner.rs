//! Pack text and its `%VAR%` substitution, porting
//! `resources_format_template`.
//!
//! This is the shell's, not the engine's, even though C keeps the formatter in
//! `resources.c`. The engine here returns outcome VALUES where C queues TEXT
//! -- `AdventureCast`, `SearchOutcome`, `SpellCast` and the rest -- so turning
//! an outcome into a sentence is entirely presentation.
//!
//! Every lookup carries its own fallback: the string C compiles in for that
//! one site. C keeps all of them in one table of defaults, which means
//! transcribing every string before any view can be ported; putting the
//! default at the call site lets each view bring only its own.

use openbounty_engine::resources::Resources;

/// One `%KEY%` substitution.
pub type Var<'a> = (&'a str, &'a str);

/// Look up `group.key`, falling back to `default` when the pack declares no
/// such string.
pub fn text<'a>(resources: &'a Resources, group: &str, key: &str, default: &'a str) -> &'a str {
    resources.string(group, key).unwrap_or(default)
}

/// Substitute `%KEY%` tokens, porting `resources_format_template`.
///
/// A `%` that does not open a token the caller supplied is emitted verbatim,
/// so a string containing a bare percent sign survives unharmed. An empty
/// token (`%%`) is likewise left alone, because C requires at least one
/// character between the delimiters.
pub fn format(template: &str, vars: &[Var<'_>]) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            // Copy whole characters, not bytes, so a multi-byte character is
            // not split.
            let rest = &template[index..];
            let ch = rest.chars().next().expect("index is a char boundary");
            out.push(ch);
            index += ch.len_utf8();
            continue;
        }
        // Find the closing delimiter of a non-empty token.
        let after = index + 1;
        let close = template[after..].find('%').map(|offset| after + offset);
        if let Some(close) = close {
            if close > after {
                let key = &template[after..close];
                if let Some((_, value)) = vars.iter().find(|(name, _)| *name == key) {
                    out.push_str(value);
                    index = close + 1;
                    continue;
                }
            }
        }
        // No token the caller knows: the percent sign is just a percent sign.
        out.push('%');
        index += 1;
    }
    out
}

/// Look up `group.key` and substitute in one step.
pub fn formatted(
    resources: &Resources,
    group: &str,
    key: &str,
    default: &str,
    vars: &[Var<'_>],
) -> String {
    format(text(resources, group, key, default), vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(strings: &str) -> Resources {
        Resources::from_json_str(&format!(r#"{{"troops": [], "strings": {strings}}}"#))
            .expect("fixture parses")
    }

    #[test]
    fn a_token_is_replaced_by_its_value() {
        assert_eq!(
            format(
                " Options / Controls / Days Left:%DAYS% ",
                &[("DAYS", "400")]
            ),
            " Options / Controls / Days Left:400 "
        );
        assert_eq!(
            format("%A% and %B%", &[("A", "one"), ("B", "two")]),
            "one and two"
        );
    }

    #[test]
    fn an_unknown_token_is_left_exactly_as_written() {
        // C emits the percent verbatim and carries on, so a template naming a
        // variable the call site does not supply shows the raw token rather
        // than swallowing the rest of the line.
        assert_eq!(format("have %GOLD% gold", &[]), "have %GOLD% gold");
        assert_eq!(
            format("%KNOWN% then %OTHER%", &[("KNOWN", "yes")]),
            "yes then %OTHER%"
        );
    }

    #[test]
    fn a_lone_percent_survives() {
        assert_eq!(format("100% sure", &[("DAYS", "4")]), "100% sure");
        assert_eq!(format("%", &[]), "%");
        // An empty token is not a token: C needs at least one character
        // between the delimiters.
        assert_eq!(format("%%", &[]), "%%");
    }

    #[test]
    fn a_token_is_matched_whole_not_by_prefix() {
        // C compares the key's length AND requires the stored key to end
        // there, so a shorter name must not match a longer token.
        assert_eq!(format("%DAYSLEFT%", &[("DAYS", "4")]), "%DAYSLEFT%");
        assert_eq!(format("%DAYS%", &[("DAYSLEFT", "4")]), "%DAYS%");
    }

    #[test]
    fn the_first_matching_variable_wins() {
        assert_eq!(format("%X%", &[("X", "first"), ("X", "second")]), "first");
    }

    #[test]
    fn a_value_containing_a_percent_is_not_rescanned() {
        // Substitution moves past what it inserted, so a value that looks
        // like a token cannot trigger another round.
        assert_eq!(format("%A%", &[("A", "%B%"), ("B", "nope")]), "%B%");
    }

    #[test]
    fn a_declared_string_wins_over_the_call_sites_default() {
        let resources = pack(r#"{"ui": {"press_esc_to_exit": "Hit ESC"}}"#);
        assert_eq!(
            text(&resources, "ui", "press_esc_to_exit", "Press 'ESC' to exit"),
            "Hit ESC"
        );
        // A key the pack omits falls back to the text C compiles in.
        assert_eq!(
            text(&resources, "ui", "quit_to_dos_prompt", " Quit (y/n) "),
            " Quit (y/n) "
        );
        // So does a group it omits entirely.
        assert_eq!(
            text(&resources, "banners", "anything", "fallback"),
            "fallback"
        );
    }

    #[test]
    fn a_pack_with_no_strings_at_all_still_reads() {
        let resources = Resources::from_json_str(r#"{"troops": []}"#).expect("parses");
        assert_eq!(
            text(&resources, "ui", "press_esc_to_exit", "default"),
            "default"
        );
        assert_eq!(
            formatted(
                &resources,
                "banners",
                "status_days_left",
                " Days Left:%DAYS% ",
                &[("DAYS", "900")]
            ),
            " Days Left:900 "
        );
    }

    #[test]
    fn a_non_string_entry_is_skipped_rather_than_breaking_the_group() {
        // `villain_descriptions` holds objects, so a group can legitimately
        // mix shapes. The string entries beside them must still read.
        let resources = pack(r#"{"ui": {"good": "yes", "bad": 7, "worse": null}}"#);
        assert_eq!(text(&resources, "ui", "good", "fallback"), "yes");
        assert_eq!(text(&resources, "ui", "bad", "fallback"), "fallback");
    }
}
