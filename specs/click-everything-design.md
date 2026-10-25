# Click-Everything Design: every action by left mouse button alone

Status: proposal. Read `AGENTS.md`, `specs/mouse-support-design.md`,
`specs/combat-mouse-design.md`, `specs/overworld-click-to-move-design.md`,
`specs/ingame-menu-design.md` first; their rules all apply.

**DIVERGENCE, deliberate.** C and the DOS game are keyboard-only. This is a
presentation-layer addition confined to the shell, in service of a future
tablet/phone build. The two rules from `mouse-support-design.md` §"Why" carry
the whole thing, plus a third this document adds:

1. A click resolves to the value the keyboard path already produces, by
   calling the same handler — so the engine's guards, order and tests stay the
   only authority on legality.
2. A view emits its hit regions from the SAME rect it draws, so the two cannot
   drift (anti-drift test is the house style).
3. **Left click alone reaches every action.** No action may require
   right-click, a modifier, hover, or a physical keyboard. Right-click and the
   keyboard stay as shortcuts that *duplicate* a left path (see §Right button).

The goal is completeness, not preference: a player with a single button and no
keyboard can do everything a keyboard player can. Each surface below is a
documented DIVERGENCE exactly like the existing mouse specs.

## Verified gap list

Re-checked against code 2026-10-25. The "18 actions" in gap 1 are the 15
`BINDINGS` in `shell/src/input.rs:260` plus `Q`/`Ctrl+Q` (`input.rs:330`) and
bare `ESC` → Game Menu (`input.rs:342`). Everything else is confirmed below.

| # | Gap | Where | Proof |
|---|---|---|---|
| 1 | 18 overworld letter actions have no click | `hud.rs` draws panels, publishes no hits | `session.rs:878` `match input.action` is key-only |
| 2 | ESC-only exits / toggles on menu views | town, home castle, own castle, recruit, gate, spells, controls | `views.rs:881,916,965,1143,771,652` and `:579`; none map ESC/Space/Left-Right |
| 3 | Prompt clicks only cover YesNo | numeric, A/B, text input | `prompt.rs:165` `click` returns `None` for non-YesNo |
| 3b | Castle recruit count entry is a second, separate digit box | `views.rs:1093` | `recruit_input` digits/Enter/Backspace, key-only |
| 4 | Win cartoon skip, fast-quit Y/N, game-over quit, bridge direction, worldmap toggle, combat Options↔Controls swap, pack picker | `session.rs:816,833,843,435,824`, `combat.rs:1546`, `pack_select.rs:30` | all `keys.*` only |
| 5 | Hero name entry | `startup.rs:440` | `update_name` reads `chars_pressed()`; declared a non-goal in `mouse-support-design.md` §Non-goals |

## Decisions

### D1. Overworld action access: clickable sidebar panels + a click-the-hero action menu

**Chosen:** two layers.

- **Sidebar panels** (`hud.rs`) become clickable for the four panels with an
  obvious single key: contract→`I`, magic→`U`, puzzle→`P`, gold purse→`V`
  (character — the Character view is where gold is shown, so the purse
  leads to the screen that explains it). The siege panel has no
  single-key equivalent (siege weapons are a *town* transaction) and stays
  inert; a comment says why. These are shortcuts, not the completeness
  guarantee.
- **Click-the-hero action menu** is the guarantee. Clicking the hero's own map
  tile opens a panel beside the hero (clamped to the map viewport), exactly as
  combat opens its action menu by clicking the acting unit
  (`combat.rs:338`, `combat-mouse-design.md` §"The action menu"). It lists all
  17 keyboard actions by key letter in a stable order; every row routes to the
  **existing `match input.action`** in `session.rs:878`, so a click is the same
  `Action` value `input::poll` would have returned. That is the strongest
  possible "click == key" test: the two paths converge on one `match`.

Rejected:

**Making the hero menu discoverable.** A tap on the hero is invisible until
someone shows it, and the target player is a child. Two cues, both shell-only:

- While no action menu has been opened yet this session, the idle overworld
  status line (the `chrome.rs` status bar, when no other `StatusMode` claims
  it) reads a pack string `strings.hints.hero_menu`, default "Click the hero
  for actions". Opening the menu once clears it for the session; it is
  session state, not saved.
- When the pointer is over the hero's own tile, the hero cell gets the same
  hover highlight a clickable row gets, so the tile reads as a button on
  desktop. On touch the status-line hint is the cue.

Tests: the hint shows on a fresh session, is gone after the first
`OverworldAction`-menu open, and never covers a `StatusMode` that already
owns the bar (fast-quit, worldmap).

- **Sidebar alone.** Five panels cannot cover 17 actions, and mapping
  army (`A`), map (`M`), options (`O`) and controls (`C`) onto a siege
  silhouette is dishonest.
- **Expand the ESC Game Menu into a command list.** `ingame-menu-design.md`
  §Decision 3 already rejected a Views submenu because it duplicates single
  keys; turning the menu into a 17-row command list re-opens that wound and
  buries turn actions (Search, End Week) behind two clicks.
- **A command bar / radial.** More new UI than the combat precedent gives us
  for free, and harder to keep from drifting.

**Order and layout.** Rows in a stable order (Views: Army, Character,
Contract, Puzzle, Map, Controls, Options; Cast Spell; Turn: Search, End Week,
Dismiss Army, Rest; Move: Fly, Land, New Continent; System: Save & Quit, Game
Menu). `Ctrl+Q` (fast quit) is omitted: it is already one left click via
Menu→Quit (`menu.rs` `MenuAction::Quit` reuses the `StatusMode::FastQuit`
confirm, `session.rs:805`). Each row is `font::GLYPH_H + 2` tall (10px), the
same `row_h` as `combat_action_menu` (`combat.rs:347`). 17 rows would be
~178px, taller than the legacy 170px map, so the menu lays out **two columns
when 17·row_h + 2·pad > map_h**, else one column — a pure function of
`Layout`, clamped inside the map rect like `combat_action_menu`
(`combat.rs:356-362`). Legacy splits (two columns of 9 and 8, ~98px tall,
fits 240×170); the modern default (672px map) does not.

**Context-sensitive rows — show, do not grey or hide.** Combat's action menu
shows every action including refusals (`combat.rs:338` lists all nine), and
that is the precedent to match: Fly and Land always appear, and clicking them
when illegal no-ops silently exactly as the key does (`game.mount_fly` /
`game.land_here` are silent on failure, `session.rs:881-886`). Greying would
require the shell to re-derive legality, which rule 1 forbids; hiding would
make the menu change shape unpredictably for a child. (`controls_visible`
hides rows in the Controls reference, but that is a *data* list, not an action
menu — the two precedents genuinely differ, and combat's is the one an action
menu should copy.)

The menu closes on a pick, on a click off the panel (backdrop, below), or on
any key, mirroring `CombatBack` / `CombatMenuClose`.

### D2. Uniform back/close: full-screen backdrop = the view's ESC, never a commit

**Chosen:** every menu view (town, home castle, own castle, recruit, gate,
spells, controls) and the action menu publish a **full-screen backdrop region
first**, then their rows, so a later row wins (`pointer.rs:106` scans in
reverse). The backdrop maps to exactly what ESC does on that surface: dismiss
for town/home/own/recruit/controls/gate/spells, cancel for a pending pick.
This is the pattern already shipped as `MenuClose` (`menu.rs:154`),
`SlotCancel`, and `CombatBack` (`combat.rs:310`), so it is the consistent
choice, not a new one.

**The one rule that makes this safe on touch:** a backdrop may only map to
BACK / CANCEL / DISMISS — never to a commit or a destructive action. A mis-tap
that lands off a row walks the view back (losing nothing: no transaction has
run), never forwards it. Commit affordances (text-input Confirm, fast-quit
Yes, the action-menu rows themselves) are always small explicit regions, never
the backdrop. This is why the fast-quit `Y` and the text-input "Confirm" are
narrow buttons, not the screen.

Rejected: **an explicit "Back" row on every view.** C's views have no such
row, so adding one is layout churn that risks parity with `views.c`'s draw
(`TOWN_ROW_COUNT`, `views.rs:208`, and friends would all move). The backdrop
reaches ESC with zero draw changes beyond one `hits.push`, which is what rule
1 demands. The status-bar hint ("Press 'ESC' to exit", `chrome.rs:21`) is
rewording-only polish for touch, out of scope for the first slices (see
Open questions).

### D3. Count/number entry without a keyboard: a digit pad, reusing the existing guards

Three number-entry surfaces, two shapes:

- **Numeric pickers** (dismiss 1–5, navigate 1–5, `Prompt::numeric`,
  `prompt.rs:88`): a **number strip 1..max_choice** drawn at the bottom of the
  prompt panel, one region per choice. Chosen over per-row body clicks because
  the dismiss body is just "Dismiss which troop?" (`session.rs:934`) — there
  is no per-row text to hit — and over a stepper because five choices are
  better named directly. A click is `PromptPick(i)` and `Prompt::click` routes
  it through the SAME `index_result` the digit keys use (`prompt.rs:209,369`).
- **A/B choice** (chest): a bottom **A / B strip** (left half = One, right
  half = Two), reusing `PromptPick(1/2)` and `index_result`, mirroring the
  shipped yes/no halves (`prompt.rs:357`). Chosen over hunting "A)" / "B)" in
  the wrapped body (fragile).
- **Count entry** (dwelling `Prompt::text_input`, `session.rs:1183`, and the
  castle recruit box, `views.rs:1093`): a **digit pad** 0–9 plus
  Backspace, Max, and Confirm, drawn in the lower part of the panel / view.
  The engine already clamps the commit (`min(max_recruitable, …)`,
  `session.rs:1824`; `count = min(count, max)`, `views.rs:1108`), so the pad
  only needs to respect the existing *length/value* guards. Those guards are
  currently inline in `Prompt::update` (`prompt.rs:228-247`) and
  `recruit_soldiers_update` (`views.rs:1126-1134`); the slice extracts them
  into `type_digit`, `backspace`, `commit`, `fill_max` helpers that BOTH the
  key path and the click path call, so click == key by construction.

Chosen over steppers (+/−/max) because dwelling and castle counts run to the
hundreds, where a stepper is tedious; the pad reaches any value in ≤3 taps and
keeps the exact value-cap guard. Steppers are noted as a possible later
addition behind the same helpers, not a substitute.

### D4. Pack picker: minimal macroquad-mouse inside `pack_select.rs`

`pack_select.rs` is the one module already allowed to name macroquad
(`AGENTS.md`, and its own header, `pack_select.rs:11`). The minimal addition:
read `is_mouse_button_pressed(MouseButton::Left)` and `mouse_position()`, and
because `draw()` already computes each row's rect (`pack_select.rs:96-100`),
hit-test `y` against those rows. A row click feeds `SelectInput { digit:
Some(row), confirm: true }` into the existing `select_step`
(`pack_select.rs:142`); a click off the rows does nothing (so no stray click
quits, matching the no-backdrop rule for class select). ESC still quits on
desktop; a touch build adds a clickable "Quit" region over the bottom hint bar
(`pack_select.rs:107`). Text and rectangles only — the exception stays narrow.

### D5. Hero name entry: compact on-screen keyboard, reusing `update_name`

**Chosen:** a compact letter grid (A–Z, space, Backspace, Done, Random) under
the name field on the CreateGame screen. `update_name` (`startup.rs:440`)
already consumes `chars_pressed`, Backspace, Enter and ESC; the buttons push
through the SAME logic (`click_name_char(ch)`, `click_name_back()`,
`click_name_confirm()`, `click_name_random()`), so the 10-char `NAME_MAX`
cap (`startup.rs:63`) and the first-letter-uppercase rule (`startup.rs:459`)
are reused verbatim. This is the decisive recommendation: a name is the one
free-text input in the game, and a child on a tablet should be able to type
one.

Rejected: **default/random only** (no typing) — smallest, but silently drops
a feature the keyboard build has, and the grid is one self-contained slice.
Rejected: **a full QWERTY on-screen keyboard** — heavier than the payoff for a
10-character name; the A–Z grid is enough and matches the fixed 8×8 font.

## Per-gap affordances

Coordinates are LOGICAL; every region is the rect the surface just drew.
Legacy geometry in the tables; modern derives from the same `Layout`
accessors, which is the point of `layout.rs`.

### Gap 1 — overworld actions

| Affordance | Where drawn | HitTarget | Routes through | click == key |
|---|---|---|---|---|
| Contract panel | `Hud::panel_rect(&layout, 0)` (`hud.rs:53`) | `HudPanel(0)` | `Action::ViewContract` in `match input.action` | `I` |
| Magic panel | panel 2 | `HudPanel(2)` | `Action::CastSpell` | `U` |
| Puzzle panel | panel 3 | `HudPanel(3)` | `Action::ViewPuzzle` | `P` |
| Gold purse panel | panel 4 | `HudPanel(4)` | `Action::ViewCharacter` | `V` |
| Hero's own map tile | the hero cell in `MapScene` (`map_render.rs`) | `MapTile(x,y)` with `(x,y)==hero` | opens the action menu | — |
| Action-menu row | beside hero, clamped to map (`combat.rs:356` pattern) | `OverworldAction(Action)` | sets `input.action`, falls into `session.rs:878` | the row's key |
| Action-menu backdrop | the map rect | `OverworldMenuClose` | close the menu | `ESC` |

`hud.rs` currently publishes no hits; it gains a `publish_hits(&mut HitList)`
called from `session.rs` right after `Hud{..}.draw` (`session.rs:1286`), so
the region is the same `panel_rect` the panel blit used. The hero tile is
already a `MapTile` region, so no new map work; `walk::click` already returns
`ClickMove::Nothing` for the hero's own tile (`walk.rs:119`), which the
session reinterprets as "open the action menu" instead of doing nothing.

### Gap 2 — menu-view back/close and missing toggles

Every region below is preceded by a full-screen backdrop mapping to that
view's ESC (D2), except where noted.

| View | Missing action | HitTarget | Handler (key it equals) |
|---|---|---|---|
| Town | ESC dismiss | `TownBack` | `views.town_update`'s `TownInput::Dismissed` (`views.rs:881` paths) |
| Home castle | ESC dismiss | `HomeCastleBack` | `HomeCastleAction::Dismiss` (`views.rs:916`) |
| Own castle | Space toggle garrison/remove | `OwnCastleToggle` | `views.own_castle_toggle_mode()` (Space) |
| Own castle | ESC dismiss | `OwnCastleBack` | `OwnCastleAction::Dismiss` |
| Recruit (phase 0) | ESC dismiss | `RecruitBack` | the `true` return of `recruit_soldiers_update` (`views.rs:1073`) |
| Recruit (error popup) | dismiss popup | reuse `RecruitBack` / backdrop | `recruit_error.clear()` (`views.rs:1067`) |
| Recruit (phase 1) | count digits | `RecruitDigit(u8)` / `RecruitBack` / `RecruitCommit` / `RecruitMax` | `recruit_soldiers_update` digit/Backspace/Enter branches (`views.rs:1093-1135`) |
| Gate | ESC cancel | `GateBack` | the cancel half of `gate_update` |
| Spells | Left/Right column | `SpellColumnLeft` / `SpellColumnRight` | the Left/Right branch of `spells_update` (`views.rs:617`) |
| Spells | ESC cancel | `SpellsBack` | `views.dismiss()` |
| Controls | ESC/C close | `ControlsBack` | `views.dismiss()` (`session.rs:556`) |

Each `*Back`/`*Toggle`/`SpellColumn*` target is resolved in the existing
`*_click` method (`town_click`, `home_castle_click`, `own_castle_click`,
`recruit_click`, `gate_click`, `spells_click`) so the click path and the key
path share the guard order — the same discipline combat already applies
(`combat-mouse-design.md` §3). The backdrop reuses one `HitTarget::ViewBack`
plus the view identity, rather than a new variant per view, if that is simpler
to match on; the design does not care which.

### Gap 3 — prompt clicks

| Prompt kind | Affordance | HitTarget | click == key |
|---|---|---|---|
| YesNo | (already shipped) left/right halves | `PromptYes` / `PromptNo` (`prompt.rs:357`) | `Y`/`N`/Enter |
| Numeric | number strip 1..max | `PromptPick(i)` | `DIGITS[i+1]` → `index_result` |
| AbChoice | A/B strip | `PromptPick(1)` / `PromptPick(2)` | `A`/`B` |
| TextInput | digit pad + Back + Max + Confirm | `PromptDigit(d)`, `PromptBack`, `PromptMax`, `PromptCommit` | `update`'s digit/Backspace/Max/Enter branches |

`Prompt::click` (`prompt.rs:165`) widens from YesNo-only to a `match` over
these, dispatching to the extracted `type_digit`/`backspace`/`commit`/
`fill_max` helpers. `Prompt::draw` (`prompt.rs:261`) publishes the strip/pad
regions, so the anti-drift test pins them to the drawn glyphs.

### Gap 4 — one-off screens

| Screen | Affordance | HitTarget | Handler |
|---|---|---|---|
| Win cartoon | full-screen skip | `WinCartoonSkip` | `views.finish_win_cartoon()` (`session.rs:817`) |
| Fast-quit confirm | status-bar left/right halves | `FastQuitYes` / `FastQuitNo` | `session.rs:836-840` (`Y`/`N`) |
| Game over | full-screen "any key quits" | `GameOverQuit` | `session.rs:843` `game_over_quits` |
| Bridge direction | click an adjacent map tile | `MapTile(x,y)` (already published) | `dx,dy = tile − hero`, then `try_build_bridge` (`session.rs:438-453`) |
| Worldmap toggle | status-bar hint text (orb only) | `WorldmapToggle` | `views.toggle_whole_map()` (`session.rs:824`) |
| Combat Options↔Controls | labelled swap button over the field | `CombatSwapView` | `views.set(Controls/Options)` (`combat.rs:1546-1549`) |
| Pack picker | row click = confirm; Quit region on touch | (no `HitTarget`; local `SelectInput`) | `select_step` (`pack_select.rs:142`) |

Fast-quit and worldmap regions live in `chrome.rs`, which does not currently
publish hits: `Chrome::draw`/`draw_over` (`chrome.rs:69,83`) gain a
`hits: &mut HitList` and push the status-bar region(s) they draw
(`StatusMode::FastQuit`, `StatusMode::Worldmap { .. }`). Bridge direction
reuses the map tiles that are already hit-tested every frame (`session.rs:1282`
`scene.publish_hits`) — no new surface, just a new consumer in the
`bridge_waiting` branch.

## Right button

Current bindings, each with its left equivalent already:

| Right-click does | Left equivalent | Rule for the future |
|---|---|---|
| Combat ESC (cancels picker/menus, opens action menu when idle) | click `CombatBack` / the action-menu backdrop; click the acting unit to open the menu | `combat.rs` `PointerInput.cancel`; `combat-mouse-design.md` |
| Cancel an overworld walk | any left click also cancels (`session.rs:853`) | `walk.rs` |

**Rule:** a right click may only ever *duplicate* a left path (it is a desktop
shortcut for the user's daughter), and every new surface that binds
`Button::Right` must first ship its left equivalent and a test proving the two
agree. No new surface may depend on right-click alone.

## New `HitTarget` variants (summary)

`shell/src/pointer.rs:25` gains: `HudPanel(i32)`, `OverworldAction(Action)`,
`OverworldMenuClose`, `ViewBack`, `OwnCastleToggle`, `SpellColumnLeft`,
`SpellColumnRight`, `RecruitDigit(u8)`, `RecruitBack`, `RecruitCommit`,
`RecruitMax`, `PromptPick(i32)`, `PromptDigit(u8)`, `PromptBack`, `PromptMax`,
`PromptCommit`, `WinCartoonSkip`, `FastQuitYes`, `FastQuitNo`,
`WorldmapToggle`, `GameOverQuit`, `CombatSwapView`. (`PromptYes`/`PromptNo`,
`TownRow`, `GateRow`, `RecruitRow`, `HomeCastleRow`, `OwnCastleSlot`,
`ControlsRow`, `Spell`, `MapTile` and the combat/menu/startup variants already
exist.)

## Work order (player value first; each slice is one PR)

Each slice lands green and is committed separately; tests are headless through
`Pointer`/`Keys` and the `HitList`, as the existing mouse specs do.

1. **Overworld actions** — `hud.rs` (`publish_hits`), `session.rs` (hero-tile
   opens menu; menu rows feed `input.action`), new `action_menu` draw (a small
   function beside `hud.rs`'s patterns, or in `hud.rs`/`main.rs`), `pointer.rs`
   (`HudPanel`, `OverworldAction`, `OverworldMenuClose`), `chrome.rs` (the
   "Click the hero for actions" status hint and the hero-tile hover cue).
   Tests: anti-drift for each `HudPanel` and each menu row; dispatch
   equivalence — clicking `OverworldAction(Action::Search)` opens the same
   Search prompt as `S` (`session.rs:910`), clicking Fly matches `F`, etc., for
   all 17; hero-tile click opens the menu; a sidebar click that is not a move
   does not walk (`session.rs:853` already stops on any click).

2. **Menu-view back/close + toggles** — `views.rs` (`*_click` widening + the
   new draw regions), `session.rs` (route the new targets), `pointer.rs`.
   Tests: per view, clicking the backdrop dismisses exactly as ESC; clicking
   `OwnCastleToggle` equals Space; `SpellColumnLeft/Right` equal the arrow
   keys; `RecruitBack` equals ESC; a backdrop click never commits (dismissing
   mid-recruit leaves `recruit_whom == 0` and no troop bought).

3. **Prompt clicks (numeric, A/B, text) + castle recruit count** —
   `prompt.rs` (extract `type_digit`/`backspace`/`commit`/`fill_max`; widen
   `click`; draw strip/pad), `views.rs` (recruit pad via the same helpers),
   `pointer.rs`.
   Tests: `PromptPick(i)` == `Digit i+1` for every `max_choice`; `PromptPick`
   on A/B == `A`/`B`; digit-pad click sequence reproduces a typed count
   including the value-cap rejection (`prompt.rs:238`) and length cap
   (`prompt.rs:235`); `PromptMax` fills to `max_value`; castle `RecruitCommit`
   clamps to `recruit_recompute_max` exactly as Enter does (`views.rs:1108`).

4. **One-off screens** — `chrome.rs` (hits), `session.rs` (win-cartoon,
   fast-quit, game-over, bridge, worldmap), `combat.rs` (`CombatSwapView`),
   `pointer.rs`.
   Tests: `WinCartoonSkip` == any key; `FastQuitYes/No` == `Y`/`N`; `GameOverQuit`
   quits exactly when `game_over_quits` would; bridge click on an adjacent
   tile builds the same bridge as that arrow key (whole `Game`+`Map` compared);
   `WorldmapToggle` == Space with the orb and does nothing without it;
   `CombatSwapView` == the C/O keys in `combat_view_input`.

5. **Pack picker** — `pack_select.rs` (mouse read + hit-test), its tests.
   Tests: a row click sets `digit` + `confirm` and selects that row; a click
   off rows changes nothing; `cancel` still beats movement (`pack_select.rs:296`).

6. **Hero name entry** — `startup.rs` (letter grid draw + `click_name_*`
   routing into `update_name`), `pointer.rs`/`HitTarget` (`StartupKey(ch)`-style).
   Tests: typing "Sir" by clicking S, i, r equals `type Sir`; Backspace and
   Done equal their keys; the 10-char cap and first-letter-uppercase hold on
   the click path.

## Playtest verification

`shell/src/bin/playtest.rs` **already supports clicks** — `click X Y`,
`rclick X Y`, `point X Y` (`playtest.rs:20-23`, `:378-399`). No harness work is
needed for slices 1–4 and 6. Each slice ships a script that clicks its new
surface and `expect`s the same text a key press would show:

- Slice 1: click a sidebar panel, `expect` the view title; click the hero,
  click the Search row, `expect` the Search prompt.
- Slice 2: open a town, `click` off the menu, `expect` the overworld status
  line; `click` the own-castle Space region, `expect` the mode line flips.
- Slice 3: walk to a dwelling, click the digit pad to `25`, `expect` the
  recruit refusal/confirmation the same count typed would produce.
- Slice 4: `click` the status bar to answer fast-quit; click a map tile while
  `bridge_waiting`; click the worldmap hint to toggle.

The **pack picker** (slice 5) runs *before* the `Runner`/script pipeline
(`playtest.rs:283-341` only reaches the picker when `--pack` is ambiguous), so
it is not scriptable today. Its tests are unit tests over `SelectInput`, and a
manual run with two packs installed verifies the row click on screen.

## Open questions

- **Touch wording.** The status-bar "Press 'ESC' to exit" hint
  (`chrome.rs:21`) and the in-view hints read as keyboard-only on a touch
  build. Rewording them (or gating on a future "touch" flag) is polish that
  touches `strings.*` defaults and is out of scope for the click-completeness
  slices; decide when the touch target is real.
- **Legacy touch target size.** At legacy 2× scale a 10px row is ~20px on
  screen, below touch guidelines. The design intentionally reuses glyph-height
  rows everywhere (consistent with combat and the menu); a minimum-touch-target
  re-layout belongs to the modern/touch geometry, not here.
- **`ViewBack` vs per-view variants.** Gap 2 can be one `HitTarget::ViewBack`
  dispatched on `views.active()` or one variant per view; the slice author
  picks, the equivalence tests are identical.
