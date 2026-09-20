//! The bottom-frame message dialog, porting `src/ui.c`'s dialog state and
//! the bottom-panel renderer in `src/overlay.c`.
//!
//! A dialog is an informational message the player acknowledges: it shows a
//! header and a body in the bottom panel, word-wrapped to the panel's fixed
//! column budget, and a long body paginates. One keypress advances a page;
//! the keypress on the last page dismisses it. Ctrl+Q is checked by the
//! caller, not here.
//!
//! Unlike a [`crate::prompt::Prompt`], a dialog carries no question and no
//! answer -- only text. It is the shell's rendering of the outcome VALUES the
//! engine returns (a search result, a sign read, a foe message), where C would
//! queue a `player_io_message`.
//!
//! Nothing here touches a graphics library: [`Dialog::draw`] emits
//! [`crate::draw::DrawList`] commands, and pagination is pure arithmetic, so
//! the whole thing is testable headlessly.

use crate::draw::{DrawList, Rect};
use crate::font;
use crate::layout::{Layout, PANEL_COLS, PANEL_H, PANEL_PAD_X};
use crate::palette::{Pal, Palette};
use crate::pointer::{HitList, HitTarget};

/// Body rows per page in the bottom panel. Both the renderer and the page
/// count use this, so the pager and the display never disagree about how many
/// pages a message has.
const BODY_LINES: usize = 7;

/// The dialog's bottom panel uses the rounded style (unlike the prompt, which
/// draws a plain square box in C). See `draw_panel` in `overlay.c`.
const PAD_Y: i32 = 2;
const HEADER_GAP: i32 = 1;

/// One message dialog. A zero-value `Dialog` has nothing to show.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Dialog {
    header: String,
    body: String,
    page: usize,
    active: bool,
}

impl Dialog {
    /// Open a message. `header` is drawn bold/yellow (it may be empty for an
    /// untitled message); `body` is the text, word-wrapped and paginated.
    pub fn open(&mut self, header: &str, body: &str) {
        self.header = header.to_owned();
        self.body = body.to_owned();
        self.page = 0;
        self.active = true;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn dismiss(&mut self) {
        self.active = false;
        self.page = 0;
    }

    pub fn header_text(&self) -> &str {
        &self.header
    }

    pub fn body_text(&self) -> &str {
        &self.body
    }

    /// The current page, zero-based.
    pub fn page(&self) -> usize {
        self.page
    }

    /// How many pages the body wraps to. An empty body is still one page.
    pub fn page_count(&self) -> usize {
        let mut lines = 0;
        let mut cursor = self.body.as_str();
        while !cursor.is_empty() {
            consume_line(&mut cursor, PANEL_COLS as usize);
            lines += 1;
        }
        ((lines + BODY_LINES - 1) / BODY_LINES).max(1)
    }

    /// Advance to the next page. Returns true when there was one; false on
    /// the last page, which is the caller's signal to dismiss.
    pub fn advance(&mut self) -> bool {
        if self.page + 1 < self.page_count() {
            self.page += 1;
            true
        } else {
            false
        }
    }

    /// Same outcome `advance` returns for any key. A dialog is one big dismiss
    /// region; anything else is ignored.
    pub fn click(&mut self, target: HitTarget) -> bool {
        match target {
            HitTarget::DialogDismiss => self.advance(),
            _ => false,
        }
    }

    /// Draw the bottom panel with its current page, porting
    /// `draw_dialog_ex(DLG_MODE_BOTTOM)`. Does nothing when inactive.
    pub fn draw(
        &self,
        list: &mut DrawList,
        hits: &mut HitList,
        layout: &Layout,
        palette: &Palette,
    ) {
        if !self.active {
            return;
        }

        let x = layout.panel_x();
        let y = layout.panel_y();
        let w = layout.panel_w();
        let h = PANEL_H;
        let pad_x = PANEL_PAD_X;
        let max_chars = PANEL_COLS as usize;

        // The dialog is the rounded-panel style, unlike the prompt's square
        // box.
        list.rounded_panel(
            Rect::new(x, y, w, h),
            palette.clr(Pal::DBlue),
            palette.clr(Pal::Yellow),
        );
        // The whole panel is one dismiss region.
        hits.push(Rect::new(x, y, w, h), HitTarget::DialogDismiss);

        let tx = x + pad_x;
        let mut ty = y + PAD_Y;

        let header_rows = if self.header.is_empty() {
            0
        } else {
            1 + self.header.bytes().filter(|&b| b == b'\n').count()
        };

        if header_rows > 0 {
            let mut hp = self.header.as_str();
            for _ in 0..header_rows {
                let line = consume_line(&mut hp, max_chars);
                list.text(&line, tx, ty, palette.clr(Pal::Yellow));
                ty += font::GLYPH_H;
            }
            ty += HEADER_GAP;
        }

        // Body: word-wrap to the panel width, skip to the current page, then
        // draw up to BODY_LINES rows.
        let mut cursor = self.body.as_str();
        let mut lines_skipped = 0;
        while !cursor.is_empty() && lines_skipped < self.page * BODY_LINES {
            consume_line(&mut cursor, max_chars);
            lines_skipped += 1;
        }

        let mut lines_drawn = 0;
        while !cursor.is_empty() && lines_drawn < BODY_LINES {
            let line = consume_line(&mut cursor, max_chars);
            list.text(&line, tx, ty, palette.clr(Pal::White));
            ty += font::GLYPH_H;
            lines_drawn += 1;
        }
    }
}

/// Word-wrap helper, porting `overlay.c`'s `consume_line`. Reads as much of
/// `*cursor` as fits in `max_chars` at the fixed glyph width, stopping at word
/// boundaries, and advances the cursor past what it consumed.
///
/// The wrap keeps the trailing space on the line rather than dropping it: C's
/// `back` lands on the position just AFTER the last space, so the space stays
/// in the emitted line and the next line starts at the following word. A blank
/// line (between two newlines) still advances and yields an empty string, so
/// the pager and the display stay in agreement.
fn consume_line(cursor: &mut &str, max_chars: usize) -> String {
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
    use openbounty_engine::resources::Resources;

    fn layout() -> Layout {
        let resources = Resources::from_json_str(r#"{"render": {"mode": "legacy"}, "troops": []}"#)
            .expect("fixture parses");
        Layout::from_resources(&resources).expect("lays out")
    }

    #[test]
    fn a_fresh_dialog_is_inactive() {
        let mut dialog = Dialog::default();
        assert!(!dialog.is_active());
        assert_eq!(dialog.page_count(), 1);
        assert!(!dialog.advance());
    }

    #[test]
    fn open_sets_the_text_and_resets_the_page() {
        let mut dialog = Dialog::default();
        dialog.open("Treasure", "You found 100 gold.");
        assert!(dialog.is_active());
        assert_eq!(dialog.header_text(), "Treasure");
        assert_eq!(dialog.body_text(), "You found 100 gold.");
        assert_eq!(dialog.page(), 0);
    }

    #[test]
    fn an_empty_body_is_still_one_page() {
        let mut dialog = Dialog::default();
        dialog.open("Title", "");
        assert_eq!(dialog.page_count(), 1);
        assert!(!dialog.advance());
    }

    #[test]
    fn page_count_counts_wrapped_lines_not_newlines() {
        // 8 short lines -> 2 pages (7 + 1).
        let mut dialog = Dialog::default();
        dialog.open("", "a\nb\nc\nd\ne\nf\ng\nh");
        assert_eq!(dialog.page_count(), 2);

        // A paragraph with no newlines at all still paginates: the pager
        // counts wrapped lines, not raw newlines (the bug C's rewrite fixed).
        // Fifty four-letter words wrap well past seven rows.
        let mut wrapped = Dialog::default();
        let paragraph = vec!["word"; 50].join(" ");
        wrapped.open("", &paragraph);
        assert_eq!(wrapped.page_count(), 2);
    }

    #[test]
    fn advance_walks_pages_then_reports_the_last_one() {
        let mut dialog = Dialog::default();
        dialog.open("", "a\nb\nc\nd\ne\nf\ng\nh");

        assert!(dialog.advance());
        assert_eq!(dialog.page(), 1);
        // Now on the last page: advancing reports false.
        assert!(!dialog.advance());
        assert_eq!(dialog.page(), 1);
    }

    #[test]
    fn a_single_page_dialog_never_advances() {
        let mut dialog = Dialog::default();
        dialog.open("", "short");
        assert!(!dialog.advance());
        assert_eq!(dialog.page(), 0);
    }

    #[test]
    fn the_panel_is_drawn_rounded_with_a_yellow_border() {
        let mut dialog = Dialog::default();
        dialog.open("Treasure", "You found 100 gold.");
        let layout = layout();
        let palette = Palette::fallback();

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        dialog.draw(&mut list, &mut hits, &layout, &palette);

        assert_eq!(
            list.as_slice()[0],
            DrawCmd::RectRounded {
                rect: Rect::new(
                    layout.panel_x(),
                    layout.panel_y(),
                    layout.panel_w(),
                    PANEL_H
                ),
                roundness: crate::draw::ROUNDNESS,
                segments: crate::draw::ROUND_SEGMENTS,
                color: palette.clr(Pal::DBlue),
            }
        );
        assert_eq!(
            list.as_slice()[1],
            DrawCmd::RectRoundedLines {
                rect: Rect::new(
                    layout.panel_x(),
                    layout.panel_y(),
                    layout.panel_w(),
                    PANEL_H
                ),
                roundness: crate::draw::ROUNDNESS,
                segments: crate::draw::ROUND_SEGMENTS,
                color: palette.clr(Pal::Yellow),
            }
        );
    }

    #[test]
    fn the_header_is_yellow_and_the_body_white() {
        let mut dialog = Dialog::default();
        dialog.open("Treasure", "You found 100 gold.");
        let layout = layout();
        let palette = Palette::fallback();

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        dialog.draw(&mut list, &mut hits, &layout, &palette);

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
            .any(|(t, c)| *t == "Treasure" && *c == palette.clr(Pal::Yellow)));
        assert!(texts
            .iter()
            .any(|(t, c)| *t == "You found 100 gold." && *c == palette.clr(Pal::White)));
    }

    #[test]
    fn an_untitled_dialog_draws_no_header_row() {
        let mut dialog = Dialog::default();
        dialog.open("", "Your search of this area has\nrevealed nothing.");
        let layout = layout();
        let palette = Palette::fallback();

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        dialog.draw(&mut list, &mut hits, &layout, &palette);

        let texts = list.texts();
        assert!(!texts.iter().any(|(_, _, t)| *t == ""));
        // No yellow header line: every text is the white body.
        let yellow: Vec<_> = list
            .as_slice()
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::Text { color, .. } if *color == palette.clr(Pal::Yellow) => Some(()),
                _ => None,
            })
            .collect();
        assert!(yellow.is_empty());
    }

    #[test]
    fn a_multiline_header_draws_each_line_in_yellow() {
        let mut dialog = Dialog::default();
        dialog.open("Line One\nLine Two", "body");
        let layout = layout();
        let palette = Palette::fallback();

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        dialog.draw(&mut list, &mut hits, &layout, &palette);

        let yellow: Vec<_> = list
            .texts()
            .into_iter()
            .filter(|(_, _, t)| *t == "Line One" || *t == "Line Two")
            .map(|(_, y, t)| (y, t.to_owned()))
            .collect();
        assert_eq!(yellow.len(), 2);
        assert!(yellow[0].0 < yellow[1].0);
    }

    #[test]
    fn page_two_skips_the_first_pages_lines() {
        let mut dialog = Dialog::default();
        // 7 lines per page; page 2 shows the 8th line ("h").
        dialog.open("", "a\nb\nc\nd\ne\nf\ng\nh");
        assert_eq!(dialog.page_count(), 2);
        dialog.advance();

        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        dialog.draw(&mut list, &mut hits, &layout, &palette);

        let body: Vec<String> = list
            .texts()
            .into_iter()
            .map(|(_, _, t)| t.to_owned())
            .collect();
        assert_eq!(body, vec!["h"]);
    }

    #[test]
    fn consume_line_wraps_at_word_boundaries_and_keeps_blank_lines() {
        let mut cursor: &str = "hello world foo";
        // The wrap keeps the trailing space (C's `back` lands just after the
        // last space), and the next line starts at the following word.
        assert_eq!(consume_line(&mut cursor, 10), "hello ");
        assert_eq!(consume_line(&mut cursor, 10), "world foo");

        // A blank line between two newlines yields an empty line.
        let mut blank: &str = "a\n\nb";
        assert_eq!(consume_line(&mut blank, 10), "a");
        assert_eq!(consume_line(&mut blank, 10), "");
        assert_eq!(consume_line(&mut blank, 10), "b");
    }

    #[test]
    fn an_inactive_dialog_draws_nothing() {
        let dialog = Dialog::default();
        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        dialog.draw(&mut list, &mut hits, &layout, &palette);
        assert!(list.is_empty());
    }

    #[test]
    fn a_dialog_publishes_one_dismiss_region_covering_its_panel() {
        let layout = layout();
        let mut dialog = Dialog::default();
        dialog.open("Header", "Body");

        let mut list = DrawList::new();
        let mut hits = HitList::new();
        dialog.draw(&mut list, &mut hits, &layout, &Palette::fallback());

        let region = hits.regions().first().expect("one region");
        assert_eq!(region.target, HitTarget::DialogDismiss);
        let cx = region.rect.x + region.rect.w / 2;
        let cy = region.rect.y + region.rect.h / 2;
        assert_eq!(hits.hit(cx, cy), Some(HitTarget::DialogDismiss));
    }

    #[test]
    fn clicking_a_dialog_advances_it_like_any_key() {
        let mut by_key = Dialog::default();
        by_key.open("Header", "Body");
        let keyed = by_key.advance();

        let mut by_click = Dialog::default();
        by_click.open("Header", "Body");
        let clicked = by_click.click(HitTarget::DialogDismiss);

        assert_eq!(clicked, keyed);
    }
}
