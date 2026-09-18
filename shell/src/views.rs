//! Full-screen view overlays, porting `views.c`'s stack and the read-only
//! views of `views_render.c`.
//!
//! A view covers the map area (and, for the character view, the sidebar too)
//! with its own panel while the overworld keeps running underneath. Views are
//! stacked so a sub-screen can ESC back to its parent; the top of the stack is
//! the active view and an empty stack is the overworld.
//!
//! Nothing here touches a graphics library: each view emits
//! [`crate::draw::DrawList`] commands, so the layout is testable headlessly
//! the way the engine's combat digests are.

use openbounty_engine::game::Game;
use openbounty_engine::resources::Resources;

use crate::banner;
use crate::draw::{Color, DrawList, Dst, Rect, Sprite};
use crate::font;
use crate::layout::{Layout, MAP_X, MAP_Y};
use crate::palette::{Pal, Palette};

/// The character portrait's legacy size. C defaults to 96x102 and the shipped
/// pack's class portraits are exactly that, so the belt below it is placed a
/// deterministic 102px from the view top.
const PORTRAIT_W: i32 = 96;
const PORTRAIT_H: i32 = 102;

/// Which overlay is on top of the stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewKind {
    Character,
}

/// The view stack. `Views::default()` is the empty (overworld) stack.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Views {
    stack: Vec<ViewKind>,
}

impl Views {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> Option<ViewKind> {
        self.stack.last().copied()
    }

    pub fn is_active(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Replace the stack with a single view.
    pub fn set(&mut self, view: ViewKind) {
        self.stack.clear();
        self.stack.push(view);
    }

    /// Pop the top view. Popping an empty stack is a no-op.
    pub fn dismiss(&mut self) {
        self.stack.pop();
    }

    /// True when the active view swaps the status bar to "Press 'ESC' to
    /// exit". The character view always does.
    pub fn wants_exit_hint(&self) -> bool {
        matches!(self.active(), Some(ViewKind::Character))
    }

    /// Draw the active view over the map area.
    pub fn draw(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        game: &Game,
    ) {
        match self.active() {
            Some(ViewKind::Character) => draw_character(list, layout, palette, resources, game),
            None => {}
        }
    }
}

fn stat_label<'a>(resources: &'a Resources, key: &str, default: &'a str) -> &'a str {
    banner::text(resources, "stats", key, default)
}

fn stat_row(
    list: &mut DrawList,
    label: &str,
    count: i32,
    label_x: i32,
    val_right: i32,
    y: &mut i32,
    white: Color,
) {
    list.text(label, label_x, *y, white);
    // A zero value draws no number, matching C.
    if count != 0 {
        let value = count.to_string();
        list.text(&value, val_right - font::measure_width(&value), *y, white);
    }
    *y += font::GLYPH_H;
}

/// Ports `draw_character`: portrait on the left, stat rows on the right, and
/// the artifact/map belt below.
fn draw_character(
    list: &mut DrawList,
    layout: &Layout,
    palette: &Palette,
    resources: &Resources,
    game: &Game,
) {
    let vx = MAP_X;
    let vw = layout.map_w + layout.sidebar_w;
    let view_y = MAP_Y;
    let view_h = layout.map_h;

    // Full content width (covers the sidebar), solid grey background.
    list.rect(Rect::new(vx, view_y, vw, view_h), palette.clr(Pal::DGrey));

    if let Some(cls) = resources.class_by_id(game.character.cls.id.as_str()) {
        list.blit(
            Sprite::ClassPortrait(cls.index as usize),
            Dst::rect(vx, view_y, PORTRAIT_W, PORTRAIT_H),
        );
    }

    let sx = vx + PORTRAIT_W;
    let label_x = sx + 1;
    let val_right = vx + vw - 2;
    let white = palette.clr(Pal::White);
    let mut y = view_y + 1;

    // Name row: "Sir Rust the General".
    let name_row = format!(
        "{} the {}",
        game.character.name.as_str(),
        game.character.cls.rank_title.as_str()
    );
    list.text(&name_row, label_x, y, white);
    y += font::GLYPH_H;

    stat_row(
        list,
        stat_label(resources, "leadership", "Leadership"),
        game.stats.leadership_current,
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "commission", "Commission/Week"),
        game.stats.commission_weekly,
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "gold", "Gold"),
        game.stats.gold,
        label_x,
        val_right,
        &mut y,
        white,
    );
    y += 4;
    stat_row(
        list,
        stat_label(resources, "spell_power", "Spell power"),
        game.stats.spell_power,
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "max_spells", "Max # of spells"),
        game.stats.max_spells,
        label_x,
        val_right,
        &mut y,
        white,
    );
    y += 4;
    stat_row(
        list,
        stat_label(resources, "villains_caught", "Villains caught"),
        game.villains_caught_count(),
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "artifacts_found", "Artifacts found"),
        game.artifacts_found_count(),
        label_x,
        val_right,
        &mut y,
        white,
    );
    y += 4;
    stat_row(
        list,
        stat_label(resources, "castles_garrisoned", "Castles garrisoned"),
        game.castles_owned_count(),
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "followers_killed", "Followers killed"),
        game.stats.followers_killed,
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "current_score", "Current score"),
        game.compute_score(resources),
        label_x,
        val_right,
        &mut y,
        white,
    );

    // Inventory belt: 6 columns (4 artifacts + 2 zone maps) by 2 rows.
    let inv_x = vx;
    let inv_y = view_y + PORTRAIT_H;
    let item_w = vw / 6;
    let item_h = layout.tile_h;
    let belt_h = item_h * 2;

    list.rect(Rect::new(inv_x, inv_y, vw, belt_h), palette.clr(Pal::DRed));
    list.rect_lines(Rect::new(inv_x, inv_y, vw, belt_h), palette.clr(Pal::Grey));
    for col in 1..6 {
        list.rect(
            Rect::new(inv_x + col * item_w, inv_y, 1, belt_h),
            palette.clr(Pal::Grey),
        );
    }
    list.rect(
        Rect::new(inv_x, inv_y + item_h, vw, 1),
        palette.clr(Pal::Grey),
    );

    for index in 0..8 {
        if !game.artifacts.found[index] {
            continue;
        }
        let ix = inv_x + (index % 4) as i32 * item_w;
        let iy = inv_y + (index / 4) as i32 * item_h;
        list.blit(Sprite::ViewIcon(index), Dst::rect(ix, iy, item_w, item_h));
    }

    // Zone maps sit in columns 4 and 5, only when the zone is discovered.
    let map_x = inv_x + 4 * item_w;
    for index in 0..4 {
        if index >= game.world.zones_discovered.len() || !game.world.zones_discovered[index] {
            continue;
        }
        let ix = map_x + (index % 2) as i32 * item_w;
        let iy = inv_y + (index / 2) as i32 * item_h;
        list.blit(
            Sprite::ViewIcon(8 + index),
            Dst::rect(ix, iy, item_w, item_h),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> (Resources, Layout) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "classes": [
                  {"index":0,"id":"knight","name":"Knight","portrait":"knight.png",
                   "ranks":[{"id":"general","name":"General"}]}
                ],
                "artifacts": [
                  {"index":0,"id":"sword","name":"Sword","icon":"sword.png","power":"increased_damage","effect":"e"}
                ]}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        (resources, layout)
    }

    fn knight_game() -> Game {
        let mut game = Game::default();
        game.character.cls.id.copy_from_str("knight");
        game.character.cls.rank_title.copy_from_str("General");
        game.character.name.copy_from_str("Sir Rust");
        game.stats.leadership_current = 100;
        game.stats.commission_weekly = 20;
        game.stats.gold = 7500;
        game
    }

    #[test]
    fn the_stack_replaces_and_pops() {
        let mut views = Views::new();
        assert!(!views.is_active());
        assert_eq!(views.active(), None);

        views.set(ViewKind::Character);
        assert!(views.is_active());
        assert_eq!(views.active(), Some(ViewKind::Character));

        views.dismiss();
        assert!(!views.is_active());
        // Popping an empty stack is a no-op.
        views.dismiss();
        assert!(!views.is_active());
    }

    #[test]
    fn the_character_view_requests_the_exit_hint() {
        let mut views = Views::new();
        assert!(!views.wants_exit_hint());
        views.set(ViewKind::Character);
        assert!(views.wants_exit_hint());
    }

    #[test]
    fn the_character_view_draws_the_name_and_stats() {
        let (resources, layout) = fixtures();
        let game = knight_game();
        let palette = Palette::fallback();
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(&mut list, &layout, &palette, &resources, &game);

        let labels: Vec<&str> = list.texts().iter().map(|(_, _, t)| *t).collect();
        assert!(labels[0].starts_with("Sir Rust the General"));
        assert!(labels.contains(&"Leadership"));
        assert!(labels.contains(&"Gold"));
        assert!(labels.contains(&"7500"));
        assert!(labels.contains(&"Current score"));
        // Zero-valued stats still show their label but no number.
        assert!(labels.contains(&"Villains caught"));
        assert!(labels.contains(&"Followers killed"));
        assert!(!labels.contains(&"0"));
    }

    #[test]
    fn the_inventory_belt_draws_an_artifact_icon_when_found() {
        let (resources, layout) = fixtures();
        let mut game = knight_game();
        game.artifacts.found[0] = true;
        let palette = Palette::fallback();
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(&mut list, &layout, &palette, &resources, &game);

        assert!(!list.blits_of(Sprite::ViewIcon(0)).is_empty());
        assert!(list.blits_of(Sprite::ViewIcon(1)).is_empty());
        // The class portrait is drawn from the class index.
        assert!(!list.blits_of(Sprite::ClassPortrait(0)).is_empty());
    }

    #[test]
    fn the_pack_may_reword_a_stat_label() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "strings": {"stats": {"gold": "GP"}}}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let game = knight_game();
        let palette = Palette::fallback();
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(&mut list, &layout, &palette, &resources, &game);
        assert!(list.has_text_containing("GP"));
        assert!(!list.has_text_containing("Gold"));
    }

    #[test]
    fn an_inactive_view_draws_nothing() {
        let (resources, layout) = fixtures();
        let game = knight_game();
        let palette = Palette::fallback();
        let views = Views::new();

        let mut list = DrawList::new();
        views.draw(&mut list, &layout, &palette, &resources, &game);
        assert!(list.is_empty());
    }

    #[test]
    fn a_zero_value_draws_no_number() {
        let (resources, layout) = fixtures();
        let game = knight_game();
        let palette = Palette::fallback();
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(&mut list, &layout, &palette, &resources, &game);

        // "Followers killed" is 0, so its number must not appear.
        let texts = list.texts();
        assert!(texts.iter().any(|(_, _, t)| *t == "Followers killed"));
        assert!(!texts.iter().any(|(_, _, t)| *t == "0"));
    }
}
