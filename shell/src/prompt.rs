//! Bottom-frame modal prompts, porting `src/prompt.c`.
//!
//! One prompt is active at a time. While it is, input and drawing are
//! redirected to it and the overworld keeps rendering underneath. A prompt is
//! a state machine with no window of its own: [`Prompt::update`] polls the
//! keys until it returns something other than [`PromptResult::None`], and the
//! caller switches on the answer.
//!
//! Four kinds, matching C:
//!
//! - yes/no (`Y` / `N` / Enter);
//! - a 1..5 numeric picker;
//! - an A/B letter picker;
//! - a digit-entry box that clamps both its length and its value.
//!
//! Nothing here touches a graphics library. [`Prompt::draw`] emits
//! [`crate::draw::DrawList`] commands, so the panel geometry and the
//! word-wrap are testable headlessly. The input comes from the same
//! [`crate::input::KeySource`] the overworld uses.

use openbounty_engine::resources::Resources;

use crate::banner;
use crate::draw::{DrawList, Rect};
use crate::font;
use crate::input::{Key, KeySource, DIGITS, NUMPAD_DIGITS};
use crate::layout::{Layout, PANEL_COLS, PANEL_H, PANEL_PAD_X};
use crate::palette::{Pal, Palette};
use crate::pointer::{HitList, HitTarget};

/// What a prompt returned once answered. Ports `PromptResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptResult {
    None,
    Yes,
    No,
    Cancel,
    One,
    Two,
    Three,
    Four,
    Five,
}

/// The four prompt kinds, plus the "no prompt is up" state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Kind {
    #[default]
    None,
    YesNo,
    Numeric,
    AbChoice,
    TextInput,
}

/// Hint strings C compiles in, used when the pack declares none. The pack's
/// overrides live under `strings.prompts` (see `resources.c`).
pub const DEFAULT_TEXT_HINT: &str = "(Enter to confirm / ESC cancel)";
pub const DEFAULT_NUMERIC_RANGE_HINT: &str = "(1-%COUNT% or ESC)";
pub const DEFAULT_YES_NO_HINT: &str = "(y/n)?";
pub const DEFAULT_NUMERIC_5_HINT: &str = "(1-5 or ESC)";

/// One modal prompt. A zero-value `Prompt` has no prompt active.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Prompt {
    kind: Kind,
    header: String,
    body: String,
    /// The numeric picker's ceiling, clamped to 1..5 by [`Prompt::numeric`].
    max_choice: i32,
    /// The digit-entry box's length and value caps.
    max_digits: i32,
    max_value: i32,
    /// What has been typed so far into the digit-entry box.
    buffer: String,
}

impl Prompt {
    /// Open a yes/no prompt. `header` is drawn bold/yellow; `body` is the
    /// question, word-wrapped into the panel.
    pub fn yes_no(&mut self, header: &str, body: &str) {
        self.kind = Kind::YesNo;
        self.header = header.to_owned();
        self.body = body.to_owned();
    }

    /// Open a 1..5 numeric picker. `max_choice` is clamped to that range.
    pub fn numeric(&mut self, header: &str, body: &str, max_choice: i32) {
        self.kind = Kind::Numeric;
        self.max_choice = max_choice.clamp(1, 5);
        self.header = header.to_owned();
        self.body = body.to_owned();
    }

    /// Open an A/B letter picker. `A` answers [`PromptResult::One`], `B`
    /// answers [`PromptResult::Two`], so callers can share the numeric
    /// dispatch path.
    pub fn ab(&mut self, header: &str, body: &str) {
        self.kind = Kind::AbChoice;
        self.header = header.to_owned();
        self.body = body.to_owned();
    }

    /// Open a digit-entry prompt. `max_digits` is clamped to 1..6 and
    /// `max_value` is the largest accepted number; typing past it is rejected.
    pub fn text_input(&mut self, header: &str, body: &str, max_digits: i32, max_value: i32) {
        self.kind = Kind::TextInput;
        self.max_digits = max_digits.clamp(1, 6);
        self.max_value = max_value;
        self.buffer.clear();
        self.header = header.to_owned();
        self.body = body.to_owned();
    }

    /// The value typed into the digit-entry box, or 0 when it is empty or the
    /// prompt was not a text input.
    pub fn text_input_value(&self) -> i32 {
        self.buffer.parse().unwrap_or(0)
    }

    pub fn is_active(&self) -> bool {
        self.kind != Kind::None
    }

    /// A stable string naming the kind, for diagnostics and tests. `"none"`
    /// when no prompt is up.
    pub fn kind_str(&self) -> &'static str {
        match self.kind {
            Kind::None => "none",
            Kind::YesNo => "yes_no",
            Kind::Numeric => "numeric",
            Kind::AbChoice => "ab",
            Kind::TextInput => "text",
        }
    }

    pub fn header_text(&self) -> &str {
        &self.header
    }

    pub fn body_text(&self) -> &str {
        &self.body
    }

    pub fn dismiss(&mut self) {
        self.kind = Kind::None;
        self.header.clear();
        self.body.clear();
    }

    /// Settle a yes/no answer: dismiss and report it. Shared by the keyboard
    /// (`update`) and the pointer (`click`) so the two cannot diverge.
    fn yes_no_answer(&mut self, yes: bool) -> PromptResult {
        self.dismiss();
        if yes {
            PromptResult::Yes
        } else {
            PromptResult::No
        }
    }

    /// Same outcome `update` returns for Y / N on a yes/no prompt. The other
    /// kinds have no clickable affordance, so no regions are published for
    /// them and their clicks resolve to [`PromptResult::None`].
    pub fn click(&mut self, target: HitTarget) -> PromptResult {
        if self.kind != Kind::YesNo {
            return PromptResult::None;
        }
        match target {
            HitTarget::PromptYes => self.yes_no_answer(true),
            HitTarget::PromptNo => self.yes_no_answer(false),
            _ => PromptResult::None,
        }
    }

    /// Poll one frame of input. Returns [`PromptResult::None`] while still
    /// waiting; anything else means the prompt answered itself (and, for a
    /// text input, the typed value is left readable through
    /// [`Prompt::text_input_value`]).
    pub fn update(&mut self, keys: &dyn KeySource) -> PromptResult {
        if self.kind == Kind::None {
            return PromptResult::None;
        }

        // ESC is checked before the kind, so it cancels every prompt.
        if keys.pressed(Key::Escape) {
            self.dismiss();
            return PromptResult::Cancel;
        }

        match self.kind {
            Kind::None => PromptResult::None,
            Kind::YesNo => {
                if keys.pressed(Key::Y) {
                    return self.yes_no_answer(true);
                }
                if keys.pressed(Key::N) {
                    return self.yes_no_answer(false);
                }
                // Enter counts as "yes", as in C.
                if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) {
                    return self.yes_no_answer(true);
                }
                PromptResult::None
            }
            Kind::Numeric => {
                // C reads KEY_ONE..KEY_ONE+max and their numpad twins.
                for i in 0..self.max_choice as usize {
                    if keys.pressed(DIGITS[i + 1]) || keys.pressed(NUMPAD_DIGITS[i + 1]) {
                        self.dismiss();
                        return index_result(i);
                    }
                }
                PromptResult::None
            }
            Kind::AbChoice => {
                if keys.pressed(Key::A) {
                    self.dismiss();
                    return PromptResult::One;
                }
                if keys.pressed(Key::B) {
                    self.dismiss();
                    return PromptResult::Two;
                }
                PromptResult::None
            }
            Kind::TextInput => {
                // One digit per frame. A digit that would overrun the length
                // or the value cap is rejected without being typed.
                for d in 0..10usize {
                    let pressed = keys.pressed(DIGITS[d]) || keys.pressed(NUMPAD_DIGITS[d]);
                    if !pressed {
                        continue;
                    }
                    if self.buffer.len() >= self.max_digits as usize {
                        break;
                    }
                    let candidate = format!("{}{}", self.buffer, d);
                    if candidate.parse::<i32>().unwrap_or(i32::MAX) > self.max_value {
                        break;
                    }
                    self.buffer.push((b'0' + d as u8) as char);
                    break;
                }
                if keys.pressed(Key::Backspace) && !self.buffer.is_empty() {
                    self.buffer.pop();
                }
                if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) {
                    // Commit by kind only, leaving the buffer readable through
                    // text_input_value(); the next open zeroes it.
                    self.kind = Kind::None;
                    return PromptResult::Yes;
                }
                PromptResult::None
            }
        }
    }

    /// Draw the prompt panel into `list`, porting `prompt_draw`. Does nothing
    /// when no prompt is active.
    pub fn draw(
        &self,
        list: &mut DrawList,
        hits: &mut HitList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
    ) {
        if self.kind == Kind::None {
            return;
        }

        let row_h = font::GLYPH_H + 1;
        let pad = PANEL_PAD_X;
        let x = layout.panel_x();
        let y = layout.panel_y();
        let w = layout.panel_w();
        let h = PANEL_H;
        let max_chars = PANEL_COLS as usize;

        // Rows reserved at the bottom for the hint chrome, rendered after the
        // body: two for a text input (typed value + hint), one for a
        // yes/no or numeric hint, none for an A/B choice whose body names the
        // keys.
        let bottom_rows = match self.kind {
            Kind::TextInput => 2,
            Kind::AbChoice => 0,
            _ => 1,
        };

        list.rect(Rect::new(x, y, w, h), palette.clr(Pal::DBlue));
        list.rect_lines(Rect::new(x, y, w, h), palette.clr(Pal::Yellow));

        let mut ty = y + pad;
        if !self.header.is_empty() {
            list.text(&self.header, x + pad, ty, palette.clr(Pal::Yellow));
            ty += row_h + 2;
        }

        // Body lines advance by the glyph height (8px), NOT row_h (9px). The
        // panel's 60px content area holds exactly 7 glyph rows at 8px but
        // only 6 at 9px; C steps body text by 8 so a 7-line body fits.
        let mut cursor = self.body.as_str();
        let body_step = font::GLYPH_H;
        let body_floor = y + h - pad - bottom_rows * row_h;
        while !cursor.is_empty() && ty + body_step <= body_floor {
            let line = take_line(&mut cursor, max_chars);
            list.text(&line, x + pad, ty, palette.clr(Pal::White));
            ty += body_step;
        }

        match self.kind {
            Kind::TextInput => {
                let typed = format!("{}_", self.buffer);
                list.text_centered(
                    &typed,
                    x + w / 2,
                    y + h - row_h * 2 - 2,
                    palette.clr(Pal::White),
                );
                let hint = banner::text(resources, "prompts", "text_hint", DEFAULT_TEXT_HINT);
                list.text_centered(hint, x + w / 2, y + h - row_h - 2, palette.clr(Pal::Yellow));
            }
            Kind::Numeric if self.max_choice != 5 => {
                let hint = banner::formatted(
                    resources,
                    "prompts",
                    "numeric_range_hint",
                    DEFAULT_NUMERIC_RANGE_HINT,
                    &[("COUNT", &self.max_choice.to_string())],
                );
                list.text_centered(
                    &hint,
                    x + w / 2,
                    y + h - row_h - 2,
                    palette.clr(Pal::Yellow),
                );
            }
            Kind::AbChoice => {}
            _ => {
                let hint = if self.kind == Kind::YesNo {
                    banner::text(resources, "prompts", "yes_no_hint", DEFAULT_YES_NO_HINT)
                } else {
                    banner::text(
                        resources,
                        "prompts",
                        "numeric_5_hint",
                        DEFAULT_NUMERIC_5_HINT,
                    )
                };
                list.text_centered(hint, x + w / 2, y + h - row_h - 2, palette.clr(Pal::Yellow));
            }
        }

        // A yes/no prompt is two clickable halves of its hint row: Yes on the
        // left, No on the right, matching the "(y/n)?" order.
        if self.kind == Kind::YesNo {
            let hint_y = y + h - row_h - 2;
            hits.push(Rect::new(x, hint_y, w / 2, row_h), HitTarget::PromptYes);
            hits.push(
                Rect::new(x + w / 2, hint_y, w - w / 2, row_h),
                HitTarget::PromptNo,
            );
        }
    }
}

/// Map a numeric-picker index to its result. Ports `PROMPT_RESULT_1 + i`.
fn index_result(i: usize) -> PromptResult {
    match i {
        0 => PromptResult::One,
        1 => PromptResult::Two,
        2 => PromptResult::Three,
        3 => PromptResult::Four,
        _ => PromptResult::Five,
    }
}

/// Word-wrap helper, porting `prompt.c`'s `take_line`. Reads as much of
/// `*cursor` as fits in `max_chars` at the fixed glyph width, stopping at
/// word boundaries, and advances the cursor past what it consumed.
///
/// A word longer than `max_chars` with no space in it is hard-truncated at
/// `max_chars`: the copy loop caps there, and the wrap-back only ever lands
/// on a space, so there is nothing to back up to.
fn take_line(cursor: &mut &str, max_chars: usize) -> String {
    let bytes = cursor.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
        pos += 1;
    }

    let mut line = Vec::new();
    while pos < bytes.len() && bytes[pos] != b'\n' && line.len() < max_chars {
        line.push(bytes[pos]);
        pos += 1;
    }

    if pos < bytes.len() && bytes[pos] != b'\n' && line.len() >= max_chars {
        let mut back = line.len();
        while back > 0 && line[back - 1] != b' ' {
            back -= 1;
        }
        if back > 0 {
            let over = line.len() - back;
            pos -= over;
            line.truncate(back);
        }
    }

    if pos < bytes.len() && bytes[pos] == b'\n' {
        pos += 1;
    }

    *cursor = &cursor[pos..];
    String::from_utf8_lossy(&line).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::DrawCmd;
    use crate::input::Keys;

    fn pack(strings: &str) -> Resources {
        Resources::from_json_str(&format!(
            r#"{{"render": {{"mode": "legacy"}}, "troops": [], "strings": {strings}}}"#
        ))
        .expect("fixture parses")
    }

    fn legacy_layout(resources: &Resources) -> Layout {
        Layout::from_resources(resources).expect("lays out")
    }

    #[test]
    fn a_fresh_prompt_is_inactive() {
        let mut prompt = Prompt::default();
        assert!(!prompt.is_active());
        assert_eq!(prompt.kind_str(), "none");
        assert_eq!(prompt.update(&Keys::pressed(&[Key::Y])), PromptResult::None);
    }

    #[test]
    fn yes_no_answers_y_n_and_enter_and_escape_cancels() {
        let mut prompt = Prompt::default();
        prompt.yes_no("Search...", "Are you sure?");

        assert_eq!(prompt.kind_str(), "yes_no");
        assert_eq!(prompt.header_text(), "Search...");
        assert_eq!(prompt.body_text(), "Are you sure?");

        // A key that is not bound leaves the prompt up.
        assert_eq!(prompt.update(&Keys::pressed(&[Key::S])), PromptResult::None);
        assert!(prompt.is_active());

        assert_eq!(prompt.update(&Keys::pressed(&[Key::Y])), PromptResult::Yes);
        assert!(!prompt.is_active());

        prompt.yes_no("h", "b");
        assert_eq!(prompt.update(&Keys::pressed(&[Key::N])), PromptResult::No);

        prompt.yes_no("h", "b");
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Enter])),
            PromptResult::Yes
        );

        prompt.yes_no("h", "b");
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::KpEnter])),
            PromptResult::Yes
        );

        prompt.yes_no("h", "b");
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Escape])),
            PromptResult::Cancel
        );
        assert!(!prompt.is_active());
    }

    #[test]
    fn escape_wins_even_when_an_answer_key_is_held() {
        // C checks ESC before the kind, so an ESC pressed alongside Y still
        // cancels.
        let mut prompt = Prompt::default();
        prompt.yes_no("h", "b");
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Y, Key::Escape])),
            PromptResult::Cancel
        );
    }

    #[test]
    fn a_numeric_picker_reads_digits_and_the_numpad() {
        let mut prompt = Prompt::default();
        prompt.numeric("Pick", "one", 3);
        assert_eq!(prompt.kind_str(), "numeric");

        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit1])),
            PromptResult::One
        );
        prompt.numeric("Pick", "one", 3);
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit3])),
            PromptResult::Three
        );
        prompt.numeric("Pick", "one", 3);
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Kp2])),
            PromptResult::Two
        );
        // Out of range: 3 choices, digit 4 is ignored.
        prompt.numeric("Pick", "one", 3);
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit4])),
            PromptResult::None
        );
        assert!(prompt.is_active());
    }

    #[test]
    fn a_numeric_picker_clamps_its_ceiling_to_five() {
        let mut prompt = Prompt::default();
        prompt.numeric("h", "b", 99);
        // 6 choices were asked for, but the picker only has 5, so digit 6
        // cannot be read and 5 is the top answer.
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit6])),
            PromptResult::None
        );
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit5])),
            PromptResult::Five
        );

        prompt.numeric("h", "b", 0);
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit1])),
            PromptResult::One
        );
    }

    #[test]
    fn an_ab_choice_answers_one_or_two() {
        let mut prompt = Prompt::default();
        prompt.ab("Treasure", "A) gold  B) leadership");
        assert_eq!(prompt.kind_str(), "ab");

        assert_eq!(prompt.update(&Keys::pressed(&[Key::A])), PromptResult::One);
        prompt.ab("h", "b");
        assert_eq!(prompt.update(&Keys::pressed(&[Key::B])), PromptResult::Two);
    }

    #[test]
    fn a_text_input_accepts_digits_up_to_the_value_cap() {
        let mut prompt = Prompt::default();
        prompt.text_input("How many", "Recruit how many", 3, 50);
        assert_eq!(prompt.kind_str(), "text");

        // Typing one digit per frame.
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit4])),
            PromptResult::None
        );
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit5])),
            PromptResult::None
        );
        assert_eq!(prompt.text_input_value(), 45);
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Enter])),
            PromptResult::Yes
        );
        assert_eq!(prompt.text_input_value(), 45);

        // "51" would exceed the 50 cap, so the 1 after 5 is rejected.
        prompt.text_input("h", "b", 3, 50);
        prompt.update(&Keys::pressed(&[Key::Digit5]));
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit1])),
            PromptResult::None
        );
        assert_eq!(prompt.text_input_value(), 5);
    }

    #[test]
    fn a_text_input_stops_at_its_digit_count() {
        let mut prompt = Prompt::default();
        prompt.text_input("h", "b", 2, 99);
        prompt.update(&Keys::pressed(&[Key::Digit9]));
        prompt.update(&Keys::pressed(&[Key::Digit9]));
        // The third digit is dropped: the buffer is already 2 long.
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Digit9])),
            PromptResult::None
        );
        assert_eq!(prompt.text_input_value(), 99);
    }

    #[test]
    fn a_text_input_backspaces_and_reads_the_numpad() {
        let mut prompt = Prompt::default();
        prompt.text_input("h", "b", 4, 9999);
        prompt.update(&Keys::pressed(&[Key::Kp7]));
        prompt.update(&Keys::pressed(&[Key::Digit8]));
        assert_eq!(prompt.text_input_value(), 78);
        prompt.update(&Keys::pressed(&[Key::Backspace]));
        assert_eq!(prompt.text_input_value(), 7);
        // Backspace on an empty buffer does nothing.
        prompt.update(&Keys::pressed(&[Key::Backspace]));
        prompt.update(&Keys::pressed(&[Key::Backspace]));
        assert_eq!(prompt.text_input_value(), 0);
    }

    #[test]
    fn an_empty_text_input_reads_zero() {
        let mut prompt = Prompt::default();
        prompt.text_input("h", "b", 4, 9999);
        assert_eq!(
            prompt.update(&Keys::pressed(&[Key::Enter])),
            PromptResult::Yes
        );
        assert_eq!(prompt.text_input_value(), 0);
    }

    #[test]
    fn the_panel_draws_blue_with_a_yellow_border_at_the_layout_position() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let palette = Palette::fallback();
        let mut prompt = Prompt::default();
        prompt.yes_no("Search...", "It will take some days.");

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(&mut list, &mut hits, &layout, &palette, &resources);

        assert_eq!(
            list.as_slice()[0],
            DrawCmd::Rect {
                rect: Rect::new(
                    layout.panel_x(),
                    layout.panel_y(),
                    layout.panel_w(),
                    PANEL_H
                ),
                color: palette.clr(Pal::DBlue),
            }
        );
        assert_eq!(
            list.as_slice()[1],
            DrawCmd::RectLines {
                rect: Rect::new(
                    layout.panel_x(),
                    layout.panel_y(),
                    layout.panel_w(),
                    PANEL_H
                ),
                color: palette.clr(Pal::Yellow),
            }
        );
    }

    #[test]
    fn the_header_is_yellow_and_the_body_white() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let palette = Palette::fallback();
        let mut prompt = Prompt::default();
        prompt.yes_no("Search...", "It will take some days.");

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(&mut list, &mut hits, &layout, &palette, &resources);

        assert!(list.has_text_containing("Search..."));
        assert!(list.has_text_containing("It will take"));
        // The hint line for a yes/no prompt is the pack's or the default.
        assert!(list.has_text_containing("(y/n)?"));

        // Header is drawn in yellow, the body in white.
        let texts: Vec<_> = list
            .as_slice()
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::Text { text, color, .. } => Some((text.as_str(), *color)),
                _ => None,
            })
            .collect();
        assert!(texts
            .iter()
            .any(|(t, c)| *t == "Search..." && *c == palette.clr(Pal::Yellow)));
        assert!(texts
            .iter()
            .any(|(t, c)| t.starts_with("It will take") && *c == palette.clr(Pal::White)));
    }

    #[test]
    fn the_body_wraps_at_word_boundaries_within_the_panel() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let palette = Palette::fallback();
        let mut prompt = Prompt::default();
        prompt.yes_no("h", "one two three four five six seven eight nine ten");

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(&mut list, &mut hits, &layout, &palette, &resources);

        let body: Vec<_> = list
            .texts()
            .into_iter()
            .filter(|(_, _, t)| *t != "h")
            .filter(|(_, _, t)| *t != "(y/n)?")
            .map(|(_, _, t)| t.to_owned())
            .collect();
        // Every wrapped line stays within the panel's column budget.
        assert!(!body.is_empty());
        for line in &body {
            assert!(line.len() <= PANEL_COLS as usize, "line too long: {line:?}");
        }
    }

    #[test]
    fn a_long_word_is_hard_truncated_because_c_only_wraps_on_spaces() {
        // The copy loop caps at max_chars, and the wrap-back only lands on a
        // space, so a word longer than the line is cut rather than split.
        let mut cursor: &str = "supercalifragilistic word";
        assert_eq!(take_line(&mut cursor, 10), "supercalif");
    }

    #[test]
    fn take_line_skips_leading_space_and_stops_at_newlines() {
        let mut text: &str = "  hello\nworld";
        assert_eq!(take_line(&mut text, 30), "hello");
        assert_eq!(take_line(&mut text, 30), "world");
    }

    #[test]
    fn the_numeric_range_hint_carries_the_ceiling() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let palette = Palette::fallback();
        let mut prompt = Prompt::default();
        prompt.numeric("Go to which continent?", "1 a 2 b 3 c", 3);

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(&mut list, &mut hits, &layout, &palette, &resources);

        assert!(list.has_text_containing("(1-3 or ESC)"));
    }

    #[test]
    fn a_full_five_numeric_picker_uses_the_five_hint() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let palette = Palette::fallback();
        let mut prompt = Prompt::default();
        prompt.numeric("Dismiss Army", "1 militia", 5);

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(&mut list, &mut hits, &layout, &palette, &resources);

        assert!(list.has_text_containing("(1-5 or ESC)"));
    }

    #[test]
    fn a_text_input_draws_the_typed_value_with_a_caret_and_its_hint() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let palette = Palette::fallback();
        let mut prompt = Prompt::default();
        prompt.text_input("Recruit Soldiers", "How Many", 3, 50);
        prompt.update(&Keys::pressed(&[Key::Digit2]));

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(&mut list, &mut hits, &layout, &palette, &resources);

        assert!(list.has_text_containing("2_"));
        assert!(list.has_text_containing("(Enter to confirm / ESC cancel)"));
    }

    #[test]
    fn an_ab_choice_draws_no_hint_line() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let palette = Palette::fallback();
        let mut prompt = Prompt::default();
        prompt.ab("Treasure", "A) gold  B) leadership");

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(&mut list, &mut hits, &layout, &palette, &resources);

        assert!(!list.has_text_containing("(y/n)?"));
        assert!(!list.has_text_containing("(1-5 or ESC)"));
    }

    #[test]
    fn the_pack_may_reword_the_hints() {
        let resources =
            pack(r#"{"prompts": {"yes_no_hint": "yes?"}, "dialog_titles": {"search": "Dig?"}}"#);
        let layout = legacy_layout(&resources);
        let palette = Palette::fallback();
        let mut prompt = Prompt::default();
        prompt.yes_no("Search...", "body");

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(&mut list, &mut hits, &layout, &palette, &resources);
        assert!(list.has_text_containing("yes?"));
    }

    #[test]
    fn an_inactive_prompt_draws_nothing() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let palette = Palette::fallback();
        let prompt = Prompt::default();

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(&mut list, &mut hits, &layout, &palette, &resources);
        assert!(list.is_empty());
    }

    #[test]
    fn clicking_yes_matches_pressing_y() {
        let mut by_key = Prompt::default();
        by_key.yes_no("Header", "Body");
        let keyed = by_key.update(&Keys::pressed(&[Key::Y]));

        let mut by_click = Prompt::default();
        by_click.yes_no("Header", "Body");
        let clicked = by_click.click(HitTarget::PromptYes);

        assert_eq!(keyed, PromptResult::Yes);
        assert_eq!(clicked, keyed);
    }

    #[test]
    fn a_yes_no_prompt_publishes_two_halves_left_yes_right_no() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let mut prompt = Prompt::default();
        prompt.yes_no("Header", "Body");

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(
            &mut list,
            &mut hits,
            &layout,
            &Palette::fallback(),
            &resources,
        );

        let yes = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::PromptYes)
            .expect("yes region");
        let no = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::PromptNo)
            .expect("no region");
        assert!(yes.rect.x < no.rect.x, "Yes is the left half");
        // A point in each half resolves to its own target.
        assert_eq!(
            hits.hit(yes.rect.x + 1, yes.rect.y + 1),
            Some(HitTarget::PromptYes)
        );
        assert_eq!(
            hits.hit(no.rect.x + 1, no.rect.y + 1),
            Some(HitTarget::PromptNo)
        );
    }
}
