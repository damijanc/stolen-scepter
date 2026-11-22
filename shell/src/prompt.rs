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

use crate::banner;
use crate::draw::{DrawList, Rect};
use crate::font;
use crate::input::{Key, KeySource, DIGITS, NUMPAD_DIGITS};
use crate::layout::{Layout, PANEL_COLS, PANEL_H, PANEL_PAD_X};
use crate::numpad::{self, PadKey};
use crate::palette::{Pal, Palette};
use crate::pointer::{HitList, HitTarget};
use crate::sheet::{self, Columns, Sheet, SheetButton};
use openbounty_engine::resources::Resources;

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

/// The yes/no hint C compiles in, used on desktop when the pack declares none.
pub const DEFAULT_YES_NO_HINT: &str = "(y/n)?";

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
    /// What the mobile sheet's buttons say: a numbered picker's options by
    /// name (button `i` answers choice `i + 1`), or the A/B choice's two.
    /// Empty: the buttons fall back to the numbers or letters.
    labels: Vec<String>,
    /// The mobile sheet's page, when its options do not fit on one.
    page: usize,
    /// A count's mobile keypad note, such as its price (see
    /// [`Prompt::set_note`]).
    note: String,
    /// Mobile's picture step before a count (a den): these lines and Hire /
    /// Leave in a band under the picture the view draws. Empty: no step.
    showcase: Vec<String>,
}

impl Prompt {
    /// Open a yes/no prompt. `header` is drawn bold/yellow; `body` is the
    /// question, word-wrapped into the panel.
    pub fn yes_no(&mut self, header: &str, body: &str) {
        self.reset_sheet();
        self.kind = Kind::YesNo;
        self.header = header.to_owned();
        self.body = body.to_owned();
    }

    /// Open a 1..5 numeric picker. `max_choice` is clamped to that range.
    pub fn numeric(&mut self, header: &str, body: &str, max_choice: i32) {
        self.reset_sheet();
        self.kind = Kind::Numeric;
        self.max_choice = max_choice.clamp(1, 5);
        self.header = header.to_owned();
        self.body = body.to_owned();
    }

    /// Open an A/B letter picker. `A` answers [`PromptResult::One`], `B`
    /// answers [`PromptResult::Two`], so callers can share the numeric
    /// dispatch path.
    pub fn ab(&mut self, header: &str, body: &str) {
        self.reset_sheet();
        self.kind = Kind::AbChoice;
        self.header = header.to_owned();
        self.body = body.to_owned();
    }

    /// Open a digit-entry prompt. `max_digits` is clamped to 1..6 and
    /// `max_value` is the largest accepted number; typing past it is rejected.
    pub fn text_input(&mut self, header: &str, body: &str, max_digits: i32, max_value: i32) {
        self.reset_sheet();
        self.kind = Kind::TextInput;
        self.max_digits = max_digits.clamp(1, 6);
        self.max_value = max_value;
        self.buffer.clear();
        self.header = header.to_owned();
        self.body = body.to_owned();
    }

    /// [`Prompt::numeric`] whose mobile buttons name the options: button `i`
    /// says `labels[i]` and answers choice `i + 1`, as digit `i + 1` does.
    /// The keyboard still answers 1..`max_choice`.
    pub fn numeric_labeled(
        &mut self,
        header: &str,
        body: &str,
        max_choice: i32,
        labels: Vec<String>,
    ) {
        self.numeric(header, body, max_choice);
        self.labels = labels;
        self.labels.truncate(self.max_choice as usize);
    }

    /// [`Prompt::ab`] whose two mobile buttons say `labels` (A, then B).
    pub fn ab_labeled(&mut self, header: &str, body: &str, labels: [String; 2]) {
        self.ab(header, body);
        self.labels = labels.to_vec();
    }

    fn reset_sheet(&mut self) {
        self.labels.clear();
        self.page = 0;
        self.note.clear();
        self.showcase.clear();
    }

    /// Mobile: show `lines` and Hire / Leave under the view's picture before
    /// the count keypad. Set after [`Prompt::text_input`]. Hire moves on to
    /// the keypad; Leave cancels exactly as the keypad's Back does.
    pub fn set_showcase(&mut self, lines: &[String]) {
        self.showcase = lines.to_vec();
    }

    /// A line the mobile keypad shows under its title, such as a price: the
    /// keypad covers the screen that asked. Set after [`Prompt::text_input`].
    pub fn set_note(&mut self, note: &str) {
        self.note = note.to_owned();
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
        match (self.kind, target) {
            (Kind::YesNo, HitTarget::PromptYes) => self.yes_no_answer(true),
            (Kind::YesNo, HitTarget::PromptNo) => self.yes_no_answer(false),
            (Kind::Numeric, HitTarget::PromptPick(i)) if i < self.max_choice as usize => {
                self.dismiss();
                index_result(i)
            }
            (Kind::AbChoice, HitTarget::PromptPick(i)) if i < 2 => {
                self.dismiss();
                index_result(i)
            }
            (Kind::Numeric | Kind::AbChoice, HitTarget::SheetNext) => {
                self.page += 1;
                PromptResult::None
            }
            (Kind::Numeric | Kind::AbChoice, HitTarget::SheetPrev) => {
                self.page = self.page.saturating_sub(1);
                PromptResult::None
            }
            (Kind::Numeric | Kind::AbChoice, HitTarget::PromptCancel) => {
                self.dismiss();
                PromptResult::Cancel
            }
            (Kind::TextInput, HitTarget::ShowcaseHire) => {
                self.showcase.clear();
                PromptResult::None
            }
            (Kind::TextInput, HitTarget::Pad(key)) => match key {
                PadKey::Digit(d) => {
                    self.type_digit(d);
                    PromptResult::None
                }
                PadKey::Del => {
                    self.backspace();
                    PromptResult::None
                }
                PadKey::Max => {
                    self.fill_max();
                    PromptResult::None
                }
                PadKey::Ok => self.commit(),
                PadKey::Esc => {
                    self.dismiss();
                    PromptResult::Cancel
                }
            },
            _ => PromptResult::None,
        }
    }

    /// Type one digit into the count box. A digit that would overrun the
    /// length or the value cap is rejected without being typed.
    fn type_digit(&mut self, d: u8) {
        if self.buffer.len() >= self.max_digits as usize {
            return;
        }
        let candidate = format!("{}{}", self.buffer, d);
        if candidate.parse::<i32>().unwrap_or(i32::MAX) > self.max_value {
            return;
        }
        self.buffer.push(char::from(b'0' + d));
    }

    fn backspace(&mut self) {
        self.buffer.pop();
    }

    /// Fill the count box with the most it can take: the value cap, or all
    /// nines when the cap is wider than the box.
    fn fill_max(&mut self) {
        let box_max = 10_i64.pow(self.max_digits as u32) - 1;
        self.buffer = i64::from(self.max_value).clamp(0, box_max).to_string();
    }

    /// Commit by kind only, leaving the buffer readable through
    /// `text_input_value()`; the next open zeroes it.
    fn commit(&mut self) -> PromptResult {
        self.kind = Kind::None;
        PromptResult::Yes
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
                // One digit per frame.
                for d in 0..10u8 {
                    if keys.pressed(DIGITS[d as usize]) || keys.pressed(NUMPAD_DIGITS[d as usize]) {
                        self.type_digit(d);
                        break;
                    }
                }
                if keys.pressed(Key::Backspace) {
                    self.backspace();
                }
                if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) {
                    return self.commit();
                }
                PromptResult::None
            }
        }
    }

    /// Whether both A and B begin a line of the body as it will be drawn
    /// (wrapped, and cut at the panel floor with no rows reserved). Only then
    /// can each be tapped on its own line; otherwise the A/B strip is used.
    fn ab_options_start_drawn_lines(&self, y: i32, h: i32) -> bool {
        let mut ty = y + PANEL_PAD_X;
        if !self.header.is_empty() {
            ty += font::GLYPH_H + 1 + 2;
        }
        let floor = y + h - PANEL_PAD_X;
        let mut cursor = self.body.as_str();
        let mut seen = [false; 2];
        while !cursor.is_empty() && ty + font::GLYPH_H <= floor {
            let line = take_line(&mut cursor, PANEL_COLS as usize);
            if let Some(choice) = banner::choice_index(&line) {
                seen[choice] = true;
            }
            ty += font::GLYPH_H;
        }
        seen == [true, true]
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
        if layout.touch {
            self.draw_sheet(list, hits, layout, palette);
            return;
        }

        let row_h = font::GLYPH_H + 1;
        let pad = PANEL_PAD_X;
        let x = layout.panel_x();
        let y = layout.panel_y();
        let w = layout.panel_w();
        let h = PANEL_H;
        let max_chars = PANEL_COLS as usize;

        // An A/B body that lists its options as "A) ..." / "B) ..." lines is
        // tapped on those lines; one that does not gets a two-cell strip.
        let ab_lines = self.kind == Kind::AbChoice && self.ab_options_start_drawn_lines(y, h);

        // Rows reserved at the bottom for the hint chrome, rendered after the
        // body: two for a text input (typed value + hint), one for a yes/no
        // hint or a choice strip, none for an A/B choice whose body names
        // (and is tapped on) its options.
        let bottom_rows = match self.kind {
            Kind::TextInput => 2,
            Kind::AbChoice if ab_lines => 0,
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
        // The option a wrapped line belongs to: set by its "A)" / "B)" line and
        // carried onto the continuation lines below it.
        let mut option = None;
        while !cursor.is_empty() && ty + body_step <= body_floor {
            let line = take_line(&mut cursor, max_chars);
            list.text(&line, x + pad, ty, palette.clr(Pal::White));
            if ab_lines {
                if line.trim().is_empty() {
                    // A blank line closes the option above it.
                    option = None;
                }
                if let Some(choice) = banner::choice_index(&line) {
                    option = Some(choice);
                }
                if let Some(choice) = option {
                    hits.push(
                        Rect::new(x, ty, w, body_step),
                        HitTarget::PromptPick(choice),
                    );
                }
            }
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
                // DIVERGENCE: the tappable count strip takes C's hint row.
                let strip = Rect::new(x + pad, y + h - row_h - 2, w - 2 * pad, row_h);
                numpad::draw(list, hits, strip, palette);
            }
            Kind::Numeric => {
                let labels: Vec<String> = (1..=self.max_choice).map(|n| n.to_string()).collect();
                choice_strip(
                    list,
                    hits,
                    x + pad,
                    y + h - row_h - 2,
                    w - 2 * pad,
                    &labels,
                    palette,
                );
            }
            Kind::AbChoice if !ab_lines => {
                let labels = ["A".to_owned(), "B".to_owned()];
                choice_strip(
                    list,
                    hits,
                    x + pad,
                    y + h - row_h - 2,
                    w - 2 * pad,
                    &labels,
                    palette,
                );
            }
            Kind::AbChoice => {}
            _ if !layout.touch => {
                // Desktop keeps C's line: the pack's hint, centred, whose two
                // halves are Yes and No for the mouse.
                let hint = banner::text(resources, "prompts", "yes_no_hint", DEFAULT_YES_NO_HINT);
                let hint_y = y + h - row_h - 2;
                list.text_centered(hint, x + w / 2, hint_y, palette.clr(Pal::Yellow));
                hits.push(Rect::new(x, hint_y, w / 2, row_h), HitTarget::PromptYes);
                hits.push(
                    Rect::new(x + w / 2, hint_y, w - w / 2, row_h),
                    HitTarget::PromptNo,
                );
            }
            _ => {
                // DIVERGENCE: C draws a "(y/n)?" hint here. The row is two
                // framed buttons instead, Yes on the left and No on the right
                // in that order; each fills its half, so a tap has a large,
                // visible target, and the labels keep the keys.
                let hint_y = y + h - row_h - 2;
                let left = x + pad;
                let mid = x + w / 2;
                let right = x + w - pad;
                for (rect, label, target) in [
                    (
                        Rect::new(left, hint_y, mid - 1 - left, row_h),
                        "Y:YES",
                        HitTarget::PromptYes,
                    ),
                    (
                        Rect::new(mid + 1, hint_y, right - mid - 1, row_h),
                        "N:NO",
                        HitTarget::PromptNo,
                    ),
                ] {
                    list.button(
                        rect,
                        label,
                        palette.clr(Pal::Blue),
                        palette.clr(Pal::Yellow),
                    );
                    hits.push(rect, target);
                }
            }
        }
    }
}

/// A row of equal cells, one per label, each published as
/// `PromptPick(index)`. The numbered picker and a bare A/B prompt answer here:
/// the labels name the keys, so the strip doubles as the keyboard hint.
fn choice_strip(
    list: &mut DrawList,
    hits: &mut HitList,
    x: i32,
    y: i32,
    w: i32,
    labels: &[String],
    palette: &Palette,
) {
    // One cell per label plus a closing ESC cell, which cancels as the key
    // does and keeps the "or ESC" hint the row used to carry.
    let n = labels.len() as i32 + 1;
    let cells = labels
        .iter()
        .enumerate()
        .map(|(i, label)| (label.as_str(), HitTarget::PromptPick(i)))
        .chain(std::iter::once(("ESC", HitTarget::PromptCancel)));
    for (i, (label, target)) in cells.enumerate() {
        let left = x + w * i as i32 / n;
        let right = x + w * (i as i32 + 1) / n;
        let rect = Rect::new(left, y, right - left, font::GLYPH_H + 1);
        list.button(
            rect,
            label,
            palette.clr(Pal::Blue),
            palette.clr(Pal::Yellow),
        );
        hits.push(rect, target);
    }
}

impl Prompt {
    /// The touch build's prompt: a big-button sheet, or the keypad for a
    /// count (`specs/mobile-ui-design.md`). Every button carries the target
    /// the desktop strip or halves publish, so `click` answers it unchanged.
    fn draw_sheet(
        &self,
        list: &mut DrawList,
        hits: &mut HitList,
        layout: &Layout,
        palette: &Palette,
    ) {
        if self.kind == Kind::TextInput && !self.showcase.is_empty() {
            sheet::showcase(
                list,
                hits,
                layout,
                palette,
                crate::views::SHOWCASE_TOP,
                &self.showcase,
                &[
                    ("Hire", HitTarget::ShowcaseHire),
                    ("Leave", HitTarget::Pad(PadKey::Esc)),
                ],
            );
            return;
        }
        if self.kind == Kind::TextInput {
            sheet::keypad(
                list,
                hits,
                layout,
                palette,
                &self.header,
                &self.note,
                &self.buffer,
                Some(self.max_value),
            );
            return;
        }
        let button = |label: String, target: HitTarget| SheetButton { label, target };
        let (body, buttons, back) = match self.kind {
            Kind::YesNo => (
                self.body.clone(),
                vec![
                    button("Yes".to_owned(), HitTarget::PromptYes),
                    button("No".to_owned(), HitTarget::PromptNo),
                ],
                None,
            ),
            Kind::Numeric => {
                let labels: Vec<String> = if self.labels.is_empty() {
                    (1..=self.max_choice).map(|n| n.to_string()).collect()
                } else {
                    self.labels.clone()
                };
                let buttons = labels
                    .into_iter()
                    .enumerate()
                    .map(|(i, label)| button(label, HitTarget::PromptPick(i)))
                    .collect();
                // Named buttons ARE the "1. Zone" lines, so those leave the
                // body (decision 7: names, not numbers).
                let body = if self.labels.is_empty() {
                    self.body.clone()
                } else {
                    self.body
                        .lines()
                        .filter(|line| banner::plain_label(line) == line.trim())
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                (body, buttons, Some(HitTarget::PromptCancel))
            }
            _ => {
                // The A/B body lists its options as "A) ..." lines; with named
                // buttons those lines ARE the buttons, so the body stops above
                // them (spec decision 5: no letter prefixes on mobile).
                let (body, labels) = if self.labels.len() == 2 {
                    let intro: Vec<&str> = self
                        .body
                        .lines()
                        .take_while(|line| banner::choice_index(line).is_none())
                        .collect();
                    (intro.join("\n"), self.labels.clone())
                } else {
                    (self.body.clone(), vec!["A".to_owned(), "B".to_owned()])
                };
                let buttons = labels
                    .into_iter()
                    .enumerate()
                    .map(|(i, label)| button(label, HitTarget::PromptPick(i)))
                    .collect();
                (body, buttons, Some(HitTarget::PromptCancel))
            }
        };
        let sheet = Sheet {
            key: "prompt",
            title: self.header.clone(),
            body,
            buttons,
            columns: Columns::One,
            back,
        };
        let page = self
            .page
            .min(sheet::page_count(layout, &sheet).saturating_sub(1));
        sheet::draw(list, hits, layout, palette, &sheet, page);
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
    use openbounty_engine::resources::Resources;

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

    /// DIVERGENCE: C draws a "(1-N or ESC)" hint under a numbered picker.
    /// The tappable digit strip takes that row instead; its labels name the
    /// keys, and ESC (Android Back) still cancels.
    #[test]
    fn the_numeric_strip_replaces_the_key_hint() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.numeric("Go to which continent?", "1 a 2 b 3 c", 3);
        let (list, hits) = drawn(&prompt, &resources);
        assert!(!list.has_text_containing("or ESC"));
        assert_eq!(picks(&hits).len(), 3);
    }

    #[test]
    fn a_text_input_draws_the_typed_value_with_a_caret_and_the_count_strip() {
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
        // DIVERGENCE: the count strip takes C's "(Enter to confirm / ESC
        // cancel)" row; its OK and < name what Enter and Backspace do.
        assert!(list.has_text_containing("OK"));
        assert!(!list.has_text_containing("Enter to confirm"));
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

    /// On the touch build the yes/no buttons fill their halves of the row,
    /// so each is a visible tap target; the region is the block drawn.
    #[test]
    fn touch_yes_no_row_is_two_buttons() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.yes_no("Search...", "body");
        let (list, hits) = drawn_on(&prompt, &resources, true);
        for target in [HitTarget::PromptYes, HitTarget::PromptNo] {
            let rect = hits
                .regions()
                .iter()
                .find(|r| r.target == target)
                .map(|r| r.rect)
                .expect("button region");
            assert!(
                list.as_slice()
                    .iter()
                    .any(|cmd| matches!(cmd, DrawCmd::Rect { rect: r, .. } if *r == Rect::new(rect.x + 1, rect.y, rect.w - 2, rect.h))),
                "{target:?} is drawn where it is tapped"
            );
        }
        assert!(!list.has_text_containing("(y/n)"));
    }

    /// Desktop keeps C's yes/no line: the pack's hint, centred, and its two
    /// halves clickable as Yes and No, with no button drawn.
    #[test]
    fn desktop_yes_no_shows_the_hint_with_halves() {
        let resources =
            pack(r#"{"prompts": {"yes_no_hint": "yes?"}, "dialog_titles": {"search": "Dig?"}}"#);
        let mut prompt = Prompt::default();
        prompt.yes_no("Search...", "body");
        let (list, hits) = drawn(&prompt, &resources);
        assert!(list.has_text_containing("yes?"));
        assert!(!list.has_text_containing("Y:YES"));
        let yes = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::PromptYes)
            .expect("yes");
        let no = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::PromptNo)
            .expect("no");
        let layout = legacy_layout(&resources);
        assert_eq!(yes.rect.x, layout.panel_x());
        assert_eq!(yes.rect.w, layout.panel_w() / 2);
        assert_eq!(no.rect.x, layout.panel_x() + layout.panel_w() / 2);
        assert!(!list
            .as_slice()
            .iter()
            .any(|cmd| matches!(cmd, DrawCmd::Rect { rect, .. } if rect.y == yes.rect.y)));
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

    fn drawn(prompt: &Prompt, resources: &Resources) -> (DrawList, HitList) {
        drawn_on(prompt, resources, false)
    }

    /// Drawn on the touch build (`touch`) or the desktop one.
    fn drawn_on(prompt: &Prompt, resources: &Resources, touch: bool) -> (DrawList, HitList) {
        let mut layout = legacy_layout(resources);
        layout.touch = touch;
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        prompt.draw(
            &mut list,
            &mut hits,
            &layout,
            &Palette::fallback(),
            resources,
        );
        (list, hits)
    }

    fn picks(hits: &HitList) -> Vec<(Rect, usize)> {
        hits.regions()
            .iter()
            .filter_map(|r| match r.target {
                HitTarget::PromptPick(i) => Some((r.rect, i)),
                _ => None,
            })
            .collect()
    }

    /// The shipped chest body, with its numbers filled in.
    const CHEST_BODY: &str = "Beneath an old stone you\nfind a buried hoard.\nYou may:\nA) Keep the 400 gold.\nB) Share it with the local\nfolk, which raises your\nleadership by 40.";

    #[test]
    fn numeric_pick_click_equals_digit_key() {
        for max in 1..=5 {
            for i in 0..max as usize {
                let mut by_key = Prompt::default();
                by_key.numeric("h", "b", max);
                let keyed = by_key.update(&Keys::pressed(&[DIGITS[i + 1]]));
                let mut by_click = Prompt::default();
                by_click.numeric("h", "b", max);
                assert_eq!(by_click.click(HitTarget::PromptPick(i)), keyed);
                assert!(!by_click.is_active());
            }
            let mut beyond = Prompt::default();
            beyond.numeric("h", "b", max);
            assert_eq!(
                beyond.click(HitTarget::PromptPick(max as usize)),
                PromptResult::None
            );
            assert!(beyond.is_active());
        }
    }

    #[test]
    fn ab_pick_click_equals_a_and_b_keys() {
        for (i, key) in [(0, Key::A), (1, Key::B)] {
            let mut by_key = Prompt::default();
            by_key.ab("", CHEST_BODY);
            let keyed = by_key.update(&Keys::pressed(&[key]));
            let mut by_click = Prompt::default();
            by_click.ab("", CHEST_BODY);
            assert_eq!(by_click.click(HitTarget::PromptPick(i)), keyed);
            assert!(!by_click.is_active());
        }
    }

    #[test]
    fn numeric_strip_has_one_cell_per_choice_left_to_right() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.numeric("Dismiss", "Dismiss which troop?", 3);
        let (list, hits) = drawn(&prompt, &resources);
        let cells = picks(&hits);
        assert_eq!(cells.iter().map(|c| c.1).collect::<Vec<_>>(), vec![0, 1, 2]);
        let layout = legacy_layout(&resources);
        let panel = Rect::new(
            layout.panel_x(),
            layout.panel_y(),
            layout.panel_w(),
            PANEL_H,
        );
        for (i, (rect, _)) in cells.iter().enumerate() {
            assert!(rect.x >= panel.x && rect.x + rect.w <= panel.x + panel.w);
            assert!(rect.y >= panel.y && rect.y + rect.h <= panel.y + panel.h);
            let digit = (i + 1).to_string();
            assert!(list.as_slice().iter().any(|cmd| matches!(cmd,
                DrawCmd::Text { text, x, .. } if *text == digit && *x >= rect.x && *x < rect.x + rect.w)));
        }
    }

    #[test]
    fn ab_choice_lines_are_the_tap_targets() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.ab("", CHEST_BODY);
        let (list, hits) = drawn(&prompt, &resources);
        let line_y = |needle: &str| {
            list.as_slice()
                .iter()
                .find_map(|cmd| match cmd {
                    DrawCmd::Text { text, y, .. } if text.starts_with(needle) => Some(*y),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{needle} drawn"))
        };
        let at = |y: i32| hits.hit(hits.regions()[0].rect.x + 1, y + 1);
        assert_eq!(at(line_y("Beneath")), None);
        assert_eq!(at(line_y("You may")), None);
        assert_eq!(at(line_y("A) Keep")), Some(HitTarget::PromptPick(0)));
        assert_eq!(at(line_y("B) Share")), Some(HitTarget::PromptPick(1)));
        assert_eq!(at(line_y("leadership by")), Some(HitTarget::PromptPick(1)));
    }

    #[test]
    fn ab_body_without_choice_lines_gets_a_strip() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.ab("", "Take the gold or the glory?");
        let (_, hits) = drawn(&prompt, &resources);
        let cells = picks(&hits);
        assert_eq!(cells.iter().map(|c| c.1).collect::<Vec<_>>(), vec![0, 1]);
        assert_eq!(cells[0].0.y, cells[1].0.y, "two halves of one row");
    }

    /// Line taps need BOTH options to start a drawn line; otherwise one of
    /// them would have no target, so the strip is used instead.
    #[test]
    fn ab_lines_fall_back_to_the_strip_unless_both_options_start_a_drawn_line() {
        let resources = pack("{}");
        let long = format!("{}A) Keep it.\nB) Give it away.", "Filler line\n".repeat(7));
        for body in ["A) gold  B) leadership", long.as_str()] {
            let mut prompt = Prompt::default();
            prompt.ab("", body);
            let (_, hits) = drawn(&prompt, &resources);
            let cells = picks(&hits);
            assert_eq!(
                cells.iter().map(|c| c.1).collect::<Vec<_>>(),
                vec![0, 1],
                "{body:?}"
            );
            assert_eq!(cells[0].0.y, cells[1].0.y, "a two-cell strip for {body:?}");
        }
    }

    #[test]
    fn shipped_chest_body_fits() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.ab("", CHEST_BODY);
        let (list, _) = drawn(&prompt, &resources);
        for line in CHEST_BODY.lines() {
            assert!(list.has_text_containing(line), "{line:?} drawn");
        }
    }

    #[test]
    fn yes_no_and_numeric_publish_nothing_over_the_body() {
        let resources = pack("{}");
        for numeric in [false, true] {
            let mut prompt = Prompt::default();
            if numeric {
                prompt.numeric("h", "A) not a choice here", 2);
            } else {
                prompt.yes_no("h", "A) not a choice here");
            }
            let (list, hits) = drawn(&prompt, &resources);
            let body_y = list
                .as_slice()
                .iter()
                .find_map(|cmd| match cmd {
                    DrawCmd::Text { text, y, .. } if text.starts_with("A)") => Some(*y),
                    _ => None,
                })
                .expect("body drawn");
            assert!(hits
                .regions()
                .iter()
                .all(|r| r.rect.y + r.rect.h <= body_y || r.rect.y > body_y));
        }
    }

    fn pad(key: crate::numpad::PadKey) -> HitTarget {
        HitTarget::Pad(key)
    }

    #[test]
    fn pad_clicks_type_the_same_count_as_keys() {
        use crate::numpad::PadKey::*;
        let mut prompt = Prompt::default();
        prompt.text_input("h", "b", 3, 50);
        assert_eq!(prompt.click(pad(Digit(4))), PromptResult::None);
        assert_eq!(prompt.click(pad(Digit(2))), PromptResult::None);
        assert_eq!(prompt.text_input_value(), 42);
        // 429 > 50: rejected exactly as the key path rejects it.
        prompt.click(pad(Digit(9)));
        assert_eq!(prompt.text_input_value(), 42);
        prompt.click(pad(Del));
        assert_eq!(prompt.text_input_value(), 4);
        assert_eq!(prompt.click(pad(Ok)), PromptResult::Yes);
        assert!(!prompt.is_active());
        assert_eq!(prompt.text_input_value(), 4);

        let mut keyed = Prompt::default();
        keyed.text_input("h", "b", 3, 50);
        for key in [Key::Digit4, Key::Digit2, Key::Digit9, Key::Backspace] {
            keyed.update(&Keys::pressed(&[key]));
        }
        assert_eq!(
            keyed.update(&Keys::pressed(&[Key::Enter])),
            PromptResult::Yes
        );
        assert_eq!(keyed, prompt);
    }

    #[test]
    fn pad_max_fills_the_value_cap() {
        use crate::numpad::PadKey::*;
        let mut prompt = Prompt::default();
        prompt.text_input("h", "b", 4, 137);
        prompt.click(pad(Digit(5)));
        prompt.click(pad(Max));
        assert_eq!(prompt.click(pad(Ok)), PromptResult::Yes);
        assert_eq!(prompt.text_input_value(), 137);
    }

    /// A cap wider than the box fills the box with nines: the most the
    /// player can type, never more than the cap.
    #[test]
    fn pad_max_fills_a_narrow_box_with_nines() {
        let mut prompt = Prompt::default();
        prompt.text_input("h", "b", 4, 12345);
        prompt.click(pad(crate::numpad::PadKey::Max));
        prompt.click(pad(crate::numpad::PadKey::Ok));
        assert_eq!(prompt.text_input_value(), 9999);
    }

    /// An option's tap area ends at a blank line, so closing prose is not B.
    #[test]
    fn ab_option_areas_end_at_a_blank_line() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.ab("", "A) Keep it.\nB) Give it.\n\nChoose wisely.");
        let (list, hits) = drawn(&prompt, &resources);
        let y = list
            .as_slice()
            .iter()
            .find_map(|cmd| match cmd {
                DrawCmd::Text { text, y, .. } if text.starts_with("Choose") => Some(*y),
                _ => None,
            })
            .expect("drawn");
        assert_eq!(hits.hit(hits.regions()[0].rect.x + 1, y + 1), None);
    }

    #[test]
    fn pad_regions_are_published_only_for_text_input() {
        let resources = pack("{}");
        let layout = legacy_layout(&resources);
        let panel = Rect::new(
            layout.panel_x(),
            layout.panel_y(),
            layout.panel_w(),
            PANEL_H,
        );
        let count = |hits: &HitList| {
            hits.regions()
                .iter()
                .filter(|r| matches!(r.target, HitTarget::Pad(_)))
                .inspect(|r| {
                    assert!(r.rect.x >= panel.x && r.rect.x + r.rect.w <= panel.x + panel.w);
                    assert!(r.rect.y >= panel.y && r.rect.y + r.rect.h <= panel.y + panel.h);
                })
                .count()
        };
        let mut prompt = Prompt::default();
        prompt.text_input("Hire", "Hire how many?", 4, 900);
        assert_eq!(
            count(&drawn(&prompt, &resources).1),
            crate::numpad::PAD_KEYS.len()
        );
        prompt.yes_no("h", "b");
        assert_eq!(count(&drawn(&prompt, &resources).1), 0);
        prompt.numeric("h", "b", 3);
        assert_eq!(count(&drawn(&prompt, &resources).1), 0);
        prompt.ab("", "A) x\nB) y");
        assert_eq!(count(&drawn(&prompt, &resources).1), 0);
    }

    #[test]
    fn pad_keys_do_nothing_on_other_prompts() {
        let mut prompt = Prompt::default();
        prompt.yes_no("h", "b");
        assert_eq!(
            prompt.click(pad(crate::numpad::PadKey::Ok)),
            PromptResult::None
        );
        assert!(prompt.is_active());
    }

    /// The strips end in an ESC cell: a tap cancels exactly as Esc does, and
    /// its label is the keyboard hint the strip's row used to carry.
    #[test]
    fn every_strip_ends_in_an_esc_cell_that_cancels_like_escape() {
        let resources = pack("{}");
        let open = |kind: usize, p: &mut Prompt| match kind {
            0 => p.numeric("h", "b", 3),
            1 => p.ab("", "Take the gold or the glory?"),
            _ => p.text_input("h", "b", 3, 50),
        };
        for kind in 0..3 {
            let mut prompt = Prompt::default();
            open(kind, &mut prompt);
            let (list, hits) = drawn(&prompt, &resources);
            assert!(list.has_text_containing("ESC"), "kind {kind} shows ESC");
            let cancel = hits
                .regions()
                .iter()
                .find(|r| {
                    matches!(
                        r.target,
                        HitTarget::PromptCancel | HitTarget::Pad(PadKey::Esc)
                    )
                })
                .map(|r| r.target)
                .expect("an ESC cell");
            let mut keyed = Prompt::default();
            open(kind, &mut keyed);
            let by_key = keyed.update(&Keys::pressed(&[Key::Escape]));
            assert_eq!(prompt.click(cancel), by_key);
            assert_eq!(by_key, PromptResult::Cancel);
            assert!(!prompt.is_active());
        }
    }

    /// The buttons: every region but the blank one a mobile sheet lays
    /// under them.
    fn tap_regions(hits: &HitList) -> Vec<(HitTarget, Rect)> {
        hits.regions()
            .iter()
            .filter(|r| r.target != HitTarget::SheetBlank)
            .map(|r| (r.target, r.rect))
            .collect()
    }

    fn all_finger_sized(hits: &HitList) {
        for (target, rect) in tap_regions(hits) {
            assert!(rect.h >= crate::sheet::MIN_BUTTON_H, "{target:?} {rect:?}");
        }
    }

    #[test]
    fn mobile_yes_no_is_a_sheet_pair() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.yes_no("Search", "It will take some days.");
        let (list, hits) = drawn_on(&prompt, &resources, true);
        all_finger_sized(&hits);
        let targets: Vec<_> = tap_regions(&hits).into_iter().map(|(t, _)| t).collect();
        assert_eq!(targets, vec![HitTarget::PromptYes, HitTarget::PromptNo]);
        assert!(list.has_text_containing("Yes") && list.has_text_containing("No"));
        assert!(!list.has_text_containing("(y/n)"));
    }

    #[test]
    fn mobile_numeric_lists_its_labels_and_cancels_with_back() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.numeric_labeled(
            "Send Troops Home",
            "Send which troop home?",
            5,
            vec!["Militia x20".to_owned(), "Archers x2".to_owned()],
        );
        let (list, hits) = drawn_on(&prompt, &resources, true);
        all_finger_sized(&hits);
        assert!(list.has_text_containing("Militia x20"));
        let targets: Vec<_> = tap_regions(&hits).into_iter().map(|(t, _)| t).collect();
        assert_eq!(
            targets,
            vec![
                HitTarget::PromptPick(0),
                HitTarget::PromptPick(1),
                HitTarget::PromptCancel
            ]
        );
        // The keyboard still answers 1-5, unchanged.
        let mut keyed = prompt.clone();
        assert_eq!(
            keyed.update(&Keys::pressed(&[DIGITS[2]])),
            PromptResult::Two
        );
    }

    #[test]
    fn mobile_chest_shows_its_labels_without_the_letter_lines() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.ab_labeled(
            "",
            CHEST_BODY,
            [
                "Keep 400 gold".to_owned(),
                "Share it (+40 leadership)".to_owned(),
            ],
        );
        let (list, hits) = drawn_on(&prompt, &resources, true);
        all_finger_sized(&hits);
        assert!(list.has_text_containing("Keep 400 gold"));
        assert!(list.has_text_containing("Beneath an old stone"));
        assert!(
            !list.has_text_containing("A) Keep"),
            "the letter lines become the buttons"
        );
        let mut clicked = prompt.clone();
        assert_eq!(clicked.click(HitTarget::PromptPick(1)), PromptResult::Two);
    }

    /// Review finding: the continent picker's "1. Zone" lines are the
    /// buttons, so the body does not list them a second time.
    #[test]
    fn mobile_named_buttons_replace_the_numbered_lines() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.numeric_labeled(
            "Navigate",
            "Sail where?\n1. Greenreach\n2. Isleward",
            2,
            vec!["Greenreach".to_owned(), "Isleward".to_owned()],
        );
        let (list, _) = drawn_on(&prompt, &resources, true);
        assert!(list.has_text_containing("Sail where?"));
        assert!(!list.has_text_containing("1. Greenreach"));
        assert!(list.has_text_containing("Greenreach"));
    }

    #[test]
    fn mobile_count_is_a_keypad() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        prompt.text_input("Archers", "Hire how many?", 4, 40);
        let (list, hits) = drawn_on(&prompt, &resources, true);
        all_finger_sized(&hits);
        let pads = hits
            .regions()
            .iter()
            .filter(|r| matches!(r.target, HitTarget::Pad(_)))
            .count();
        assert_eq!(pads, crate::numpad::PAD_KEYS.len());
        assert!(list.has_text_containing("Archers"));
    }

    #[test]
    fn a_long_numeric_list_pages() {
        let resources = pack("{}");
        let mut prompt = Prompt::default();
        let labels: Vec<String> = (0..5).map(|i| format!("Isle number {i}")).collect();
        prompt.numeric_labeled(
            "Sail to",
            "Where to? A long way across the open sea.",
            5,
            labels,
        );
        let mut seen = Vec::new();
        for _ in 0..5 {
            let (_, hits) = drawn_on(&prompt, &resources, true);
            for (t, _) in tap_regions(&hits) {
                if let HitTarget::PromptPick(i) = t {
                    seen.push(i);
                }
            }
            if !tap_regions(&hits)
                .iter()
                .any(|(t, _)| *t == HitTarget::SheetNext)
            {
                break;
            }
            assert_eq!(prompt.click(HitTarget::SheetNext), PromptResult::None);
        }
        seen.dedup();
        assert_eq!(seen, vec![0, 1, 2, 3, 4]);
    }
}
