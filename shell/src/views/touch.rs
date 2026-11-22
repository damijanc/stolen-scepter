//! The mobile face of the menu views (spec `mobile-ui-design.md`, Task 8):
//! the town, both castles, recruiting, the gate picker, the spellbook and
//! Controls drawn as big-button sheets.
//!
//! Every button publishes the target the desktop row publishes, so a tap
//! runs the view's existing `*_click` path and can reach nothing the
//! keyboard cannot. Only the town's purchase question is new, and the
//! session asks it before running the very transaction desktop runs.

use super::*;
use crate::sheet::{self, Columns, Sheet, SheetButton};

// One column throughout: two would leave eight big glyphs a line, and the
// names here (spells, troops, towns, settings) do not fit that.

/// Whether `kind` draws as a sheet on mobile.
pub(super) fn has_sheet(kind: ViewKind) -> bool {
    matches!(
        kind,
        ViewKind::Town
            | ViewKind::HomeCastle
            | ViewKind::OwnCastle
            | ViewKind::RecruitSoldiers
            | ViewKind::Gate
            | ViewKind::Spells
            | ViewKind::Controls
    )
}

fn button(label: impl Into<String>, target: HitTarget) -> SheetButton {
    SheetButton {
        label: label.into(),
        target,
    }
}

impl Views {
    /// Draw the active view as a sheet when it has one on mobile. False for a
    /// view that keeps its desktop drawing (the info screens, which close on
    /// any tap already).
    pub(super) fn draw_touch(
        &mut self,
        list: &mut DrawList,
        hits: &mut HitList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        game: &Game,
        present: &Present,
    ) -> bool {
        if !self.active().is_some_and(has_sheet) {
            return false;
        }
        // Off the sheet is the view's ESC, as the desktop backdrop is, and it
        // keeps a tap on the frame from reaching the map or the sidebar.
        hits.push(
            Rect::new(0, 0, layout.screen_w, layout.screen_h),
            HitTarget::ViewBack,
        );
        let sheet = match self.active() {
            Some(ViewKind::Town) => self.town_sheet(resources, game),
            Some(ViewKind::HomeCastle) => home_castle_sheet(resources),
            Some(ViewKind::OwnCastle) => self.own_castle_sheet(resources, game),
            Some(ViewKind::RecruitSoldiers) => {
                if self.recruit_whom != 0 && self.recruit_error.is_empty() {
                    let name = resources
                        .troop_by_index(self.recruit_pool[(self.recruit_whom - 1) as usize])
                        .map(|troop| troop.name.as_str())
                        .unwrap_or("");
                    sheet::keypad(
                        list,
                        hits,
                        layout,
                        palette,
                        &format!("How many {name}?"),
                        &self.recruit_input,
                        Some(self.recruit_max),
                    );
                    return true;
                }
                self.recruit_sheet(resources, game)
            }
            Some(ViewKind::Gate) => self.gate_sheet(),
            Some(ViewKind::Spells) => self.spells_sheet(resources, game),
            Some(ViewKind::Controls) => self.controls_sheet(resources, game, layout, present),
            _ => return false,
        };
        let page = self.pager.page_for(layout, &sheet);
        sheet::draw(list, hits, layout, palette, &sheet, page);
        true
    }

    fn town_sheet(&self, resources: &Resources, game: &Game) -> Sheet {
        let name = &self.town_display_name;
        if let Some((_, question)) = &self.town_confirm {
            return Sheet {
                key: "town_confirm",
                title: question.clone(),
                body: String::new(),
                buttons: vec![
                    button("Yes", HitTarget::PromptYes),
                    button("No", HitTarget::PromptNo),
                ],
                columns: Columns::Two,
                back: None,
            };
        }
        if self.town_info_active {
            return Sheet {
                key: "town_info",
                title: name.clone(),
                body: self.town_info_body.clone(),
                buttons: vec![button("OK", HitTarget::DialogDismiss)],
                columns: Columns::One,
                back: None,
            };
        }
        let buttons = (0..TOWN_ROW_COUNT)
            .map(|row| {
                let text =
                    crate::interact::town_row_text(resources, game, &self.town_record_key, row);
                button(
                    banner::plain_label(&text),
                    HitTarget::TownRow(TownRow::from_index(row as usize)),
                )
            })
            .collect();
        Sheet {
            key: "town",
            title: crate::interact::town_header_text(resources, name),
            body: format!("Gold: {}", game.stats.gold),
            buttons,
            columns: Columns::One,
            back: Some(HitTarget::ViewBack),
        }
    }

    fn own_castle_sheet(&self, resources: &Resources, game: &Game) -> Sheet {
        let castle_name = resources
            .castle_by_id(self.own_castle_id.as_str())
            .map(|castle| {
                if castle.name.is_empty() {
                    castle.id.as_str()
                } else {
                    castle.name.as_str()
                }
            })
            .unwrap_or(self.own_castle_id.as_str());
        let mode = |garrison| crate::interact::own_castle_mode_label(resources, garrison, true);
        let mut buttons: Vec<SheetButton> = (0..GAME_ARMY_SLOTS)
            .map(|slot| {
                let stack = if self.own_castle_garrison_mode {
                    game.army
                        .get(slot)
                        .filter(|s| !s.id.is_empty() && s.count > 0)
                        .map(|s| (s.id.as_str(), s.count))
                } else {
                    game.castles
                        .iter()
                        .find(|c| c.id.as_str() == self.own_castle_id.as_str())
                        .and_then(|c| c.garrison.get(slot))
                        .filter(|u| !u.id.is_empty() && u.count > 0)
                        .map(|u| (u.id.as_str(), u.count))
                };
                let label = match stack {
                    Some((id, count)) => {
                        let name = resources
                            .troop_by_id(id)
                            .map(|troop| troop.name.as_str())
                            .unwrap_or(id);
                        format!("{name} x{count}")
                    }
                    None => "(empty)".to_owned(),
                };
                button(label, HitTarget::OwnCastleSlot(slot))
            })
            .collect();
        // The mode switch first: it decides what every troop button does.
        buttons.insert(
            0,
            button(
                format!("Switch to: {}", mode(!self.own_castle_garrison_mode)),
                HitTarget::OwnCastleToggle,
            ),
        );
        Sheet {
            key: "own_castle",
            title: format!("Castle {castle_name}"),
            body: format!("Tap a troop to: {}", mode(self.own_castle_garrison_mode)),
            buttons,
            columns: Columns::One,
            back: Some(HitTarget::ViewBack),
        }
    }

    fn recruit_sheet(&self, resources: &Resources, game: &Game) -> Sheet {
        if !self.recruit_error.is_empty() {
            return Sheet {
                key: "recruit_error",
                title: String::new(),
                body: self.recruit_error.clone(),
                buttons: vec![button("OK", HitTarget::ViewBack)],
                columns: Columns::One,
                back: None,
            };
        }
        let lead = game.stats.leadership_current;
        let buttons = (0..self.recruit_pool_count)
            .filter_map(|i| {
                let troop = resources.troop_by_index(self.recruit_pool[i])?;
                let unreachable = troop.hit_points <= 0 || lead < troop.hit_points * 6;
                let label = if unreachable {
                    format!("{} - too strong to lead", troop.name.as_str())
                } else {
                    format!("{} - {} gold", troop.name.as_str(), troop.recruit_cost)
                };
                Some(button(label, HitTarget::RecruitRow(i)))
            })
            .collect();
        Sheet {
            key: "recruit",
            title: crate::interact::recruit_soldiers_title(resources),
            body: format!("Gold: {}", game.stats.gold),
            buttons,
            columns: Columns::One,
            back: Some(HitTarget::ViewBack),
        }
    }

    fn gate_sheet(&self) -> Sheet {
        let buttons = (0..self.gate_count)
            .filter_map(|i| {
                let destination = self.gate_dest(i)?;
                Some(button(destination.name.as_str(), HitTarget::GateRow(i)))
            })
            .collect();
        Sheet {
            key: "gate",
            title: if self.gate_is_town {
                "Town Gate"
            } else {
                "Castle Gate"
            }
            .to_owned(),
            body: String::new(),
            buttons,
            columns: Columns::One,
            back: Some(HitTarget::ViewBack),
        }
    }

    fn spells_sheet(&self, resources: &Resources, game: &Game) -> Sheet {
        let held = |range: std::ops::Range<i32>| -> Vec<(i32, String)> {
            range
                .filter_map(|i| {
                    let spell = resources.spell_by_index(i)?;
                    let count = game.spells.counts[i as usize];
                    (count > 0).then(|| (i, format!("{} x{count}", spell.name.as_str())))
                })
                .collect()
        };
        let title = banner::text(resources, "spells_view", "title", DEFAULT_SV_TITLE).to_owned();
        if self.spells_cast_mode {
            // On the overworld only the adventuring spells (7..13) cast.
            let spells = held(7..14);
            let body = if spells.is_empty() {
                "You know no spells to cast here.".to_owned()
            } else {
                String::new()
            };
            return Sheet {
                key: "spells_cast",
                title,
                body,
                buttons: spells
                    .into_iter()
                    .map(|(i, label)| button(label, HitTarget::Spell(i)))
                    .collect(),
                columns: Columns::One,
                back: Some(HitTarget::ViewBack),
            };
        }
        // The read-only spellbook: what the hero holds, as text.
        let mut body = String::new();
        for (heading, default, range) in [
            ("combat_col", DEFAULT_SV_COMBAT, 0..7),
            ("adventure_col", DEFAULT_SV_ADVENTURE, 7..14),
        ] {
            body.push_str(banner::text(resources, "spells_view", heading, default));
            body.push('\n');
            let spells = held(range);
            if spells.is_empty() {
                body.push_str("  (none)\n");
            }
            for (_, label) in spells {
                body.push_str(&format!("  {label}\n"));
            }
        }
        Sheet {
            key: "spells_read",
            title,
            body: body.trim_end().to_owned(),
            buttons: Vec::new(),
            columns: Columns::One,
            back: Some(HitTarget::ViewBack),
        }
    }

    fn controls_sheet(
        &self,
        resources: &Resources,
        game: &Game,
        layout: &Layout,
        present: &Present,
    ) -> Sheet {
        let vis = controls_visible(resources);
        let on = banner::text(resources, "controls", "on", DEFAULT_CTL_ON);
        let off = banner::text(resources, "controls", "off", DEFAULT_CTL_OFF);
        let mut buttons: Vec<SheetButton> = vis
            .iter()
            .enumerate()
            .filter_map(|(k, &index)| {
                let control = resources.control_by_index(index)?;
                let value = *game.stats.options.get(index)?;
                let shown = if control.kind.as_str() == "bool" {
                    (if value == 1 { on } else { off }).to_owned()
                } else {
                    value.to_string()
                };
                Some(button(
                    format!("{}: {shown}", control.label.as_str()),
                    HitTarget::ControlsRow(k),
                ))
            })
            .collect();
        if layout.is_modern {
            let sc = present.scale_override();
            let value = if sc <= 0 {
                "Auto".to_owned()
            } else {
                format!("{sc}x")
            };
            buttons.push(button(
                format!("Scale: {value}"),
                HitTarget::ControlsRow(vis.len()),
            ));
        }
        Sheet {
            key: "controls",
            title: banner::text(resources, "controls", "title", DEFAULT_CTL_TITLE)
                .trim()
                .to_owned(),
            body: String::new(),
            buttons,
            columns: Columns::One,
            back: Some(HitTarget::ViewBack),
        }
    }
}

fn home_castle_sheet(resources: &Resources) -> Sheet {
    let body = crate::interact::home_castle_body(resources);
    let mut title = String::new();
    let mut buttons = Vec::new();
    for line in body.split('\n') {
        match banner::choice_index(line) {
            Some(choice) => buttons.push(button(
                banner::plain_label(line),
                HitTarget::HomeCastleRow(choice),
            )),
            None if title.is_empty() && !line.trim().is_empty() => {
                title = line.trim().to_owned();
            }
            None => {}
        }
    }
    Sheet {
        key: "home_castle",
        title,
        body: String::new(),
        buttons,
        columns: Columns::One,
        back: Some(HitTarget::ViewBack),
    }
}
