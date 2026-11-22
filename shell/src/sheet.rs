//! The mobile choice sheet: every choice on a touch build drawn as big,
//! finger-sized buttons over the inner screen (`specs/mobile-ui-design.md`).
//!
//! A screen describes its options as a [`Sheet`] -- a title, optional body
//! text and buttons, each carrying the SAME `HitTarget` its desktop row
//! publishes -- and [`draw`] lays them out. The click routing is the
//! screen's own, unchanged: the sheet only decides where the targets sit and
//! how big they are. Counts use [`keypad`].

use crate::draw::{DrawList, Rect};
use crate::font::{GLYPH_H, GLYPH_W};
use crate::layout::{Layout, FRAME_BOTTOM_H, FRAME_LEFT_W, FRAME_RIGHT_W, FRAME_TOP_H};
use crate::numpad::PadKey;
use crate::palette::{Pal, Palette};
use crate::pointer::{HitList, HitTarget};

/// The smallest tap target on a touch build: about 50 dp on a Pixel 9a,
/// where the game draws 5.085 screen pixels per logical pixel.
pub const MIN_BUTTON_H: i32 = 26;
/// Button labels draw at twice the glyph size.
pub const LABEL_SCALE: i32 = 2;
const GAP: i32 = 2;
const PAD: i32 = 4;
const LABEL_H: i32 = GLYPH_H as i32 * LABEL_SCALE;
const LABEL_W: i32 = GLYPH_W as i32 * LABEL_SCALE;
/// A label's lines at most; the rest of a longer label is cut with "..".
const MAX_LABEL_LINES: usize = 2;

/// The whole inner screen, inside the frame: where every sheet draws.
pub fn area(layout: &Layout) -> Rect {
    Rect::new(
        FRAME_LEFT_W,
        FRAME_TOP_H,
        layout.screen_w - FRAME_LEFT_W - FRAME_RIGHT_W,
        layout.screen_h - FRAME_TOP_H - FRAME_BOTTOM_H,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct SheetButton {
    pub label: String,
    /// What a tap does: the target the screen's desktop row publishes.
    pub target: HitTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Columns {
    One,
    Two,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    /// Names the screen, so the pager can tell a new sheet from the same
    /// one drawn again.
    pub key: &'static str,
    pub title: String,
    pub body: String,
    pub buttons: Vec<SheetButton>,
    pub columns: Columns,
    /// The Back button's target -- the screen's Esc -- when it has one.
    pub back: Option<HitTarget>,
}

/// Which page of a long sheet is showing. Owned by the session or startup;
/// a different sheet starts again at the first page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pager {
    page: usize,
    key: &'static str,
}

impl Pager {
    /// The page to draw `sheet` at: reset when the sheet changed, clamped to
    /// its last page.
    pub fn page_for(&mut self, layout: &Layout, sheet: &Sheet) -> usize {
        if self.key != sheet.key {
            self.key = sheet.key;
            self.page = 0;
        }
        self.page = self.page.min(page_count(layout, sheet).saturating_sub(1));
        self.page
    }

    pub fn next(&mut self) {
        self.page += 1;
    }

    pub fn prev(&mut self) {
        self.page = self.page.saturating_sub(1);
    }
}

/// A label wrapped to `chars` per line, at most two lines, the second cut
/// with ".." when the label is longer still.
fn label_lines(label: &str, chars: usize) -> Vec<String> {
    let mut lines = wrap(label, chars.max(1));
    if lines.len() > MAX_LABEL_LINES {
        lines.truncate(MAX_LABEL_LINES);
        let last = &mut lines[MAX_LABEL_LINES - 1];
        let keep = chars.saturating_sub(2).min(last.chars().count());
        *last = last.chars().take(keep).collect::<String>() + "..";
    }
    lines
}

/// Word wrap to `chars` columns, honouring newlines; a word longer than a
/// line is split.
fn wrap(text: &str, chars: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let mut word = word.to_owned();
            loop {
                let len = line.chars().count();
                let need = word.chars().count() + usize::from(len > 0);
                if len + need <= chars {
                    if len > 0 {
                        line.push(' ');
                    }
                    line.push_str(&word);
                    break;
                }
                if len > 0 {
                    out.push(std::mem::take(&mut line));
                    continue;
                }
                let head: String = word.chars().take(chars).collect();
                word = word.chars().skip(chars).collect();
                out.push(head);
                if word.is_empty() {
                    break;
                }
            }
        }
        out.push(line);
    }
    out
}

fn button_height(lines: usize) -> i32 {
    if lines <= 1 {
        MIN_BUTTON_H
    } else {
        lines as i32 * LABEL_H + 10
    }
}

/// Where the buttons and the nav row go for one sheet.
struct Plan {
    /// Per page: each button's index and rect.
    pages: Vec<Vec<(usize, Rect)>>,
    /// Each button's wrapped label.
    lines: Vec<Vec<String>>,
    title_y: i32,
    body: Vec<String>,
    body_y: i32,
    nav: Rect,
}

fn plan(layout: &Layout, sheet: &Sheet) -> Plan {
    let a = area(layout);
    let x = a.x + PAD;
    let w = a.w - 2 * PAD;
    let mut y = a.y + PAD;
    let title_y = y;
    if !sheet.title.is_empty() {
        y += LABEL_H + 2 * GAP;
    }
    let body = if sheet.body.is_empty() {
        Vec::new()
    } else {
        wrap(&sheet.body, (w / GLYPH_W as i32) as usize)
    };
    let body_y = y;
    y += body.len() as i32 * GLYPH_H as i32;
    if !body.is_empty() {
        y += 2 * GAP;
    }
    let nav = Rect::new(x, a.y + a.h - PAD - MIN_BUTTON_H, w, MIN_BUTTON_H);
    let floor = nav.y - GAP;

    let cols = match sheet.columns {
        Columns::One => 1,
        Columns::Two => 2,
    };
    let col_w = (w - (cols - 1) * GAP) / cols;
    let chars = ((col_w - 8) / LABEL_W).max(1) as usize;
    let lines: Vec<Vec<String>> = sheet
        .buttons
        .iter()
        .map(|b| label_lines(&b.label, chars))
        .collect();

    let mut pages: Vec<Vec<(usize, Rect)>> = vec![Vec::new()];
    let mut row_y = y;
    for row in (0..sheet.buttons.len())
        .collect::<Vec<_>>()
        .chunks(cols as usize)
    {
        let h = row
            .iter()
            .map(|i| button_height(lines[*i].len()))
            .max()
            .unwrap_or(MIN_BUTTON_H);
        if row_y + h > floor && !pages.last().expect("a page").is_empty() {
            pages.push(Vec::new());
            row_y = y;
        }
        for (c, i) in row.iter().enumerate() {
            let rect = Rect::new(x + c as i32 * (col_w + GAP), row_y, col_w, h);
            pages.last_mut().expect("a page").push((*i, rect));
        }
        row_y += h + GAP;
    }
    Plan {
        pages,
        lines,
        title_y,
        body,
        body_y,
        nav,
    }
}

/// How many pages `sheet` needs.
pub fn page_count(layout: &Layout, sheet: &Sheet) -> usize {
    plan(layout, sheet).pages.len()
}

/// One big button: a filled block a pixel in from each side, its label's
/// lines centred in big text, published as `target`.
pub fn big_button(
    list: &mut DrawList,
    hits: &mut HitList,
    palette: &Palette,
    rect: Rect,
    lines: &[String],
    target: HitTarget,
) {
    list.rect(
        Rect::new(rect.x + 1, rect.y, rect.w - 2, rect.h),
        palette.clr(Pal::Blue),
    );
    let text_h = lines.len() as i32 * LABEL_H;
    let mut ty = rect.y + (rect.h - text_h) / 2;
    for line in lines {
        list.big_text_centered(
            line,
            rect.x + rect.w / 2,
            ty,
            palette.clr(Pal::Yellow),
            LABEL_SCALE,
        );
        ty += LABEL_H;
    }
    hits.push(rect, target);
}

/// The sheet's blue, yellow-framed backdrop over [`area`]; returns the area.
pub fn backdrop(list: &mut DrawList, layout: &Layout, palette: &Palette) -> Rect {
    let a = area(layout);
    list.rect(a, palette.clr(Pal::DBlue));
    list.rect_lines(a, palette.clr(Pal::Yellow));
    a
}

/// Draw `sheet` at `page` and publish each button's target, then the nav row:
/// Back (when the sheet has one) and Prev / Next (only when it pages).
pub fn draw(
    list: &mut DrawList,
    hits: &mut HitList,
    layout: &Layout,
    palette: &Palette,
    sheet: &Sheet,
    page: usize,
) {
    let a = backdrop(list, layout, palette);
    let plan = plan(layout, sheet);
    let page = page.min(plan.pages.len() - 1);
    if !sheet.title.is_empty() {
        list.big_text_centered(
            &sheet.title,
            a.x + a.w / 2,
            plan.title_y,
            palette.clr(Pal::Yellow),
            LABEL_SCALE,
        );
    }
    for (i, line) in plan.body.iter().enumerate() {
        list.text(
            line,
            a.x + PAD,
            plan.body_y + i as i32 * GLYPH_H as i32,
            palette.clr(Pal::White),
        );
    }
    for (i, rect) in &plan.pages[page] {
        big_button(
            list,
            hits,
            palette,
            *rect,
            &plan.lines[*i],
            sheet.buttons[*i].target,
        );
    }

    let mut nav: Vec<(&str, HitTarget)> = Vec::new();
    if let Some(back) = sheet.back {
        nav.push(("Back", back));
    }
    if plan.pages.len() > 1 {
        if page > 0 {
            nav.push(("Prev", HitTarget::SheetPrev));
        }
        if page + 1 < plan.pages.len() {
            nav.push(("Next", HitTarget::SheetNext));
        }
    }
    let n = nav.len() as i32;
    for (i, (label, target)) in nav.into_iter().enumerate() {
        let left = plan.nav.x + plan.nav.w * i as i32 / n.max(1);
        let right = plan.nav.x + plan.nav.w * (i as i32 + 1) / n.max(1);
        let rect = Rect::new(left, plan.nav.y, right - left, plan.nav.h);
        big_button(list, hits, palette, rect, &[label.to_owned()], target);
    }
}

/// A count keypad over the inner screen: `title` and the typed `value` (with
/// the most allowed, when known) on top, then a 4x4 grid -- 7 8 9 Del /
/// 4 5 6 Max / 1 2 3 Back / 0 (two wide) OK (two wide) -- publishing
/// `HitTarget::Pad`, so the screen's existing pad handling answers it.
pub fn keypad(
    list: &mut DrawList,
    hits: &mut HitList,
    layout: &Layout,
    palette: &Palette,
    title: &str,
    value: &str,
    max: Option<i32>,
) {
    let a = backdrop(list, layout, palette);
    let x = a.x + PAD;
    let w = a.w - 2 * PAD;
    let mut y = a.y + PAD;
    list.big_text_centered(
        title,
        a.x + a.w / 2,
        y,
        palette.clr(Pal::Yellow),
        LABEL_SCALE,
    );
    y += LABEL_H + 2 * GAP;
    let shown = match max {
        Some(max) => format!("{value}_   (max {max})"),
        None => format!("{value}_"),
    };
    list.big_text_centered(
        &shown,
        a.x + a.w / 2,
        y,
        palette.clr(Pal::White),
        LABEL_SCALE,
    );
    y += LABEL_H + 2 * GAP;

    let rows = 4;
    let row_h = ((a.y + a.h - PAD - y - (rows - 1) * GAP) / rows).max(MIN_BUTTON_H);
    let col_w = (w - 3 * GAP) / 4;
    let d = PadKey::Digit;
    let grid: [&[(PadKey, &str, i32)]; 4] = [
        &[
            (d(7), "7", 1),
            (d(8), "8", 1),
            (d(9), "9", 1),
            (PadKey::Del, "Del", 1),
        ],
        &[
            (d(4), "4", 1),
            (d(5), "5", 1),
            (d(6), "6", 1),
            (PadKey::Max, "Max", 1),
        ],
        &[
            (d(1), "1", 1),
            (d(2), "2", 1),
            (d(3), "3", 1),
            (PadKey::Esc, "Back", 1),
        ],
        &[(d(0), "0", 2), (PadKey::Ok, "OK", 2)],
    ];
    for (r, row) in grid.iter().enumerate() {
        let mut col = 0;
        for (key, label, span) in row.iter() {
            let rect = Rect::new(
                x + col * (col_w + GAP),
                y + r as i32 * (row_h + GAP),
                span * col_w + (span - 1) * GAP,
                row_h,
            );
            big_button(
                list,
                hits,
                palette,
                rect,
                &[(*label).to_owned()],
                HitTarget::Pad(*key),
            );
            col += span;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::DrawCmd;
    use crate::numpad::{PadKey, PAD_KEYS};
    use openbounty_engine::resources::Resources;

    fn layout() -> Layout {
        let resources = Resources::from_json_str(r#"{"render": {"mode": "legacy"}, "troops": []}"#)
            .expect("fixture parses");
        Layout::from_resources(&resources).expect("lays out")
    }

    fn sheet_of(labels: &[&str], back: bool) -> Sheet {
        Sheet {
            key: "test",
            title: "Choose".to_owned(),
            body: String::new(),
            buttons: labels
                .iter()
                .enumerate()
                .map(|(i, label)| SheetButton {
                    label: (*label).to_owned(),
                    target: HitTarget::MenuEntry(i),
                })
                .collect(),
            columns: Columns::One,
            back: back.then_some(HitTarget::MenuClose),
        }
    }

    fn drawn(sheet: &Sheet, page: usize) -> (DrawList, HitList) {
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        draw(
            &mut list,
            &mut hits,
            &layout(),
            &Palette::fallback(),
            sheet,
            page,
        );
        (list, hits)
    }

    fn inside(r: Rect, a: Rect) -> bool {
        r.x >= a.x && r.y >= a.y && r.x + r.w <= a.x + a.w && r.y + r.h <= a.y + a.h
    }

    #[test]
    fn every_button_is_at_least_min_height_and_inside_the_area() {
        let (_, hits) = drawn(&sheet_of(&["Easy", "Normal", "Hard"], true), 0);
        let area = area(&layout());
        assert_eq!(hits.regions().len(), 4, "three options and Back");
        for r in hits.regions() {
            assert!(r.rect.h >= MIN_BUTTON_H, "{:?}", r);
            assert!(inside(r.rect, area), "{:?} inside {:?}", r, area);
        }
    }

    #[test]
    fn regions_are_exactly_the_drawn_buttons() {
        let (list, hits) = drawn(&sheet_of(&["Easy", "Normal"], true), 0);
        for r in hits.regions() {
            let block = Rect::new(r.rect.x + 1, r.rect.y, r.rect.w - 2, r.rect.h);
            assert!(
                list.as_slice()
                    .iter()
                    .any(|cmd| matches!(cmd, DrawCmd::Rect { rect, .. } if *rect == block)),
                "{:?} is drawn where it is tapped",
                r.target
            );
        }
        let labels: Vec<_> = list.texts().into_iter().map(|(_, _, t)| t).collect();
        for label in ["Choose", "Easy", "Normal", "Back"] {
            assert!(labels.contains(&label), "{label} drawn");
        }
    }

    #[test]
    fn a_label_wraps_to_two_lines_then_is_cut() {
        let long = "Share all of the gold with the local folk and raise leadership";
        let (list, hits) = drawn(&sheet_of(&[long, "Keep it"], false), 0);
        let first = hits.regions()[0].rect;
        let second = hits.regions()[1].rect;
        let lines: Vec<_> = list
            .texts()
            .into_iter()
            .filter(|(x, y, _)| *x >= first.x && *y >= first.y && *y < first.y + first.h)
            .map(|(_, _, t)| t.to_owned())
            .collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[1].ends_with(".."), "{lines:?}");
        assert!(first.h > MIN_BUTTON_H);
        assert!(second.y >= first.y + first.h, "the next button sits below");
    }

    #[test]
    fn long_lists_page_and_every_option_is_reachable() {
        let labels: Vec<String> = (0..17).map(|i| format!("Action {i}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let sheet = sheet_of(&refs, true);
        let pages = page_count(&layout(), &sheet);
        assert!(pages > 1);
        let mut seen = Vec::new();
        for page in 0..pages {
            let (_, hits) = drawn(&sheet, page);
            let targets: Vec<_> = hits.regions().iter().map(|r| r.target).collect();
            assert!(
                targets.contains(&HitTarget::MenuClose),
                "Back on page {page}"
            );
            assert_eq!(targets.contains(&HitTarget::SheetPrev), page > 0);
            assert_eq!(targets.contains(&HitTarget::SheetNext), page + 1 < pages);
            seen.extend(targets.into_iter().filter_map(|t| match t {
                HitTarget::MenuEntry(i) => Some(i),
                _ => None,
            }));
        }
        assert_eq!(seen, (0..17).collect::<Vec<_>>());
    }

    #[test]
    fn the_pager_resets_when_the_sheet_changes() {
        let labels: Vec<String> = (0..17).map(|i| format!("Action {i}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let long = sheet_of(&refs, true);
        let mut other = sheet_of(&["Save", "Load"], true);
        other.key = "other";
        let layout = layout();
        let mut pager = Pager::default();
        assert_eq!(pager.page_for(&layout, &long), 0);
        pager.next();
        assert_eq!(pager.page_for(&layout, &long), 1);
        for _ in 0..10 {
            pager.next();
        }
        assert_eq!(
            pager.page_for(&layout, &long),
            page_count(&layout, &long) - 1,
            "clamped"
        );
        assert_eq!(
            pager.page_for(&layout, &other),
            0,
            "a new sheet starts at page 1"
        );
        pager.prev();
        assert_eq!(pager.page_for(&layout, &other), 0);
    }

    #[test]
    fn the_keypad_publishes_every_pad_key_at_min_height() {
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        keypad(
            &mut list,
            &mut hits,
            &layout(),
            &Palette::fallback(),
            "Archers",
            "12",
            Some(40),
        );
        let area = area(&layout());
        let mut keys: Vec<PadKey> = hits
            .regions()
            .iter()
            .map(|r| {
                assert!(r.rect.h >= MIN_BUTTON_H && inside(r.rect, area), "{r:?}");
                match r.target {
                    HitTarget::Pad(key) => key,
                    other => panic!("unexpected {other:?}"),
                }
            })
            .collect();
        keys.sort_by_key(|k| format!("{k:?}"));
        let mut want = PAD_KEYS.to_vec();
        want.sort_by_key(|k| format!("{k:?}"));
        assert_eq!(keys, want);
        assert!(list.texts().iter().any(|(_, _, t)| t.contains("12")));
    }
}
