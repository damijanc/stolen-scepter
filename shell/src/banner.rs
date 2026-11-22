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

/// [`text`] for a build that may be mobile. On a touch build the mobile
/// mirror `mobile_<group>.<key>` wins, then `mobile_default`: never the
/// desktop text, which may name keys a phone does not have (spec decision 9).
/// Desktop reads exactly what [`text`] reads.
pub fn text_on<'a>(
    resources: &'a Resources,
    touch: bool,
    group: &str,
    key: &str,
    default: &'a str,
    mobile_default: &'a str,
) -> &'a str {
    if touch {
        resources
            .string(&format!("mobile_{group}"), key)
            .unwrap_or(mobile_default)
    } else {
        text(resources, group, key, default)
    }
}

/// [`formatted`] for a build that may be mobile, as [`text_on`].
pub fn formatted_on(
    resources: &Resources,
    touch: bool,
    group: &str,
    key: &str,
    default: &str,
    mobile_default: &str,
    vars: &[Var<'_>],
) -> String {
    format(
        text_on(resources, touch, group, key, default, mobile_default),
        vars,
    )
}

/// Which choice a menu line in pack text names: `Some(0)` for a line opening
/// with `A)`, `A.`, `1)` or `1.`, `Some(1)` for `B` / `2`, `None` otherwise.
/// Pack bodies list their options this way (the home castle's lettered and
/// numbered bodies, a chest's A/B), and the tap targets follow the option
/// lines rather than line numbers, so a title above the options shifts nothing.
pub fn choice_index(line: &str) -> Option<usize> {
    let mut chars = line.trim_start().chars();
    let choice = match chars.next()?.to_ascii_uppercase() {
        'A' | '1' => 0,
        'B' | '2' => 1,
        _ => return None,
    };
    matches!(chars.next(), Some(')' | '.')).then_some(choice)
}

/// A menu line without its key letter: "B) Hire a boat" becomes "Hire a
/// boat". A big button names its choice by being tapped, so the letter only
/// points at a key a phone does not have. A line with no letter is returned
/// trimmed.
pub fn plain_label(line: &str) -> &str {
    let trimmed = line.trim();
    let mut chars = trimmed.char_indices();
    if let (Some((_, key)), Some((at, mark))) = (chars.next(), chars.next()) {
        if key.is_ascii_alphanumeric() && matches!(mark, ')' | '.') {
            return trimmed[at + 1..].trim_start();
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_label_drops_only_a_leading_key_letter() {
        assert_eq!(
            plain_label("B) Hire a boat (100/week)"),
            "Hire a boat (100/week)"
        );
        assert_eq!(plain_label("  2. Audience "), "Audience");
        assert_eq!(plain_label("The Royal Hall"), "The Royal Hall");
        assert_eq!(plain_label("Mr. Smith"), "Mr. Smith");
        assert_eq!(plain_label(""), "");
    }

    #[test]
    fn text_on_prefers_the_mobile_mirror_only_on_touch() {
        let resources = Resources::from_json_str(
            r#"{"troops": [], "strings": {
                "ui": {"a": "Press ESC", "b": "ESC close"},
                "mobile_ui": {"a": "Tap to leave"}}}"#,
        )
        .expect("fixture parses");
        assert_eq!(
            text_on(&resources, false, "ui", "a", "d", "md"),
            "Press ESC"
        );
        assert_eq!(
            text_on(&resources, true, "ui", "a", "d", "md"),
            "Tap to leave"
        );
        // No mirror: the mobile default, never the desktop text.
        assert_eq!(text_on(&resources, true, "ui", "b", "d", "md"), "md");
        assert_eq!(text_on(&resources, true, "ui", "missing", "d", "md"), "md");
        assert_eq!(text_on(&resources, false, "ui", "missing", "d", "md"), "d");
        assert_eq!(
            formatted_on(
                &resources,
                true,
                "ui",
                "missing",
                "%N%",
                "N=%N%",
                &[("N", "3")]
            ),
            "N=3"
        );
    }

    /// Decision 9: no keyboard traces on mobile. Every pack string that names
    /// a key has a mobile version that does not.
    #[test]
    fn every_key_naming_string_shown_on_mobile_has_a_mobile_version() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../packs/stolen-scepter/game.json");
        let pack: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("pack reads"))
                .expect("json");
        let strings = pack["strings"].as_object().expect("strings");
        // A letter hotkey range like "A-D" or "(A-Z)": capital, dash,
        // capital, standing alone.
        let letter_range = |s: &str| {
            let c: Vec<char> = s.chars().collect();
            c.windows(3).enumerate().any(|(i, w)| {
                let alone = |j: Option<&char>| j.map_or(true, |ch| !ch.is_ascii_alphanumeric());
                w[0].is_ascii_uppercase()
                    && w[1] == '-'
                    && w[2].is_ascii_uppercase()
                    && alone(if i == 0 { None } else { c.get(i - 1) })
                    && alone(c.get(i + 3))
            })
        };
        let names_a_key = |s: &str| {
            let lower = s.to_ascii_lowercase();
            lower.contains("(y/n)")
                || lower.contains("arrow key")
                || letter_range(s)
                || s.contains(" L to ")
                || lower
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|w| matches!(w, "esc" | "enter" | "space" | "ctrl" | "key" | "keys"))
        };
        let mut missing = Vec::new();
        for (group, entries) in strings {
            if group.starts_with("mobile_") {
                continue;
            }
            let Some(entries) = entries.as_object() else {
                continue;
            };
            for (key, value) in entries {
                let Some(text) = value.as_str() else { continue };
                if !names_a_key(text) {
                    continue;
                }
                match strings
                    .get(&format!("mobile_{group}"))
                    .and_then(|g| g.get(key))
                    .and_then(|v| v.as_str())
                {
                    Some(mobile) if !names_a_key(mobile) => {}
                    other => missing.push(format!("{group}.{key} -> {other:?}")),
                }
            }
        }
        // Castle texts (the King's audience, a castle's greeting) have no
        // mirror group; the shell rewrites their one cue, "(space)", on
        // mobile. Any other key cue there would show as is.
        for castle in pack["castles"].as_array().expect("castles") {
            for block in ["audience", "dialog"] {
                let Some(entries) = castle["special"][block].as_object() else {
                    continue;
                };
                for (key, value) in entries {
                    // This is story prose ("as you enter the hall"), so look
                    // for key cues rather than any key word.
                    let text = value
                        .as_str()
                        .unwrap_or("")
                        .replace("(space)", "(tap)")
                        .to_ascii_lowercase();
                    let cue = ["(space)", "(enter)", "(esc)", "(y/n)", "press ", "key"]
                        .iter()
                        .any(|c| text.contains(c));
                    if cue {
                        missing.push(format!("castle {block}.{key} -> {text:?}"));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "no mobile version: {missing:#?}");
    }

    #[test]
    fn choice_index_reads_letter_and_number_prefixes() {
        assert_eq!(choice_index("A) Recruit soldiers"), Some(0));
        assert_eq!(choice_index("  b) Speak with the King"), Some(1));
        assert_eq!(choice_index("1. Recruit Soldiers"), Some(0));
        assert_eq!(choice_index("2. Audience"), Some(1));
        assert_eq!(choice_index("The Royal Hall"), None);
        assert_eq!(choice_index("Bravely onward"), None);
        assert_eq!(choice_index(""), None);
    }

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
