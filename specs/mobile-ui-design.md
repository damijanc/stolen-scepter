# Mobile Touch UI Design (and the desktop restore)

Status: approved and built (branch `mobile-ui`, plan `specs/mobile-ui-plan.md`,
tasks 1-10, 2026-11-22). Read `AGENTS.md` and `specs/touch-complete-plan.md` first.

## Why

The touch build shipped on 2026-11-21 makes every action reachable by a tap,
but it is not playable with fingers: list rows and buttons are 8-10 logical
pixels tall. On the user's Pixel 9a (1080x2424, density 2.625, the game drawn
at 5.085 screen pixels per logical pixel) a row is about 45 screen pixels,
2.7 mm or 17 dp. Android's comfortable minimum is 48 dp, about 7.6 mm. The
user could not hit the difficulty rows or the chest's gold/leadership lines.

The product framing, in the user's words: "mobile is a different game in the
end than desktop. While desktop should keep as close to original, with mobile
we are flexible to bring it to modern age. Kids playing on the phone never
knew the original."

## Decisions (the user's)

1. **Rules are identical on both.** "Look and controls only for now, keep the
   rules - if we change the rules then it is no longer the game we are
   building." The engine is untouched.
2. **Desktop returns to its look and behaviour from before 2026-11-21**, with
   these exceptions kept:
   - the home-castle fix (clicks land on the "A)" / "B)" lines, not the title
     and blank lines);
   - combat Controls closes on a click;
   - clicking the chest's "A)" / "B)" lines chooses;
   - clicking the tile beside the hero picks the bridge direction;
   - clicking the status bar toggles the whole world map (with the orb);
   - a click skips the win cutscene;
   - a click quits once the game is over;
   - in combat, clicking a cell that is not a valid target cancels the pick;
   - the numbered pickers (dismiss a stack, choose a continent) keep the
     1-5 + ESC strip;
   - dwelling and castle counts keep the digit strip with MAX / OK / ESC;
   - the quit confirm keeps its Y:YES / N:NO buttons.
   Reverted on desktop (mobile keeps them): yes/no prompts go back to the
   "(y/n)?" hint line, still clickable on its left/right halves as before
   2026-11-21; the name screen loses the letter grid.
3. **Mobile keeps everything from 2026-11-21 and builds on top.**
4. **Big choice sheets on mobile** for every choice that today sits on small
   rows or strips (inventory below). Things already big enough stay as they
   are: walking on the map, the combat field, the sidebar panels, choosing a
   hero on the class art, and message boxes that close on a tap.
5. **Plain wording on mobile**, with no keyboard traces: "Yes" / "No", "Back",
   "Keep 400 gold" / "Share it (+40 leadership)" rather than Y:YES, ESC,
   "A)" / "B)" or "Press ESC".
6. **Confirm before spending in towns on mobile**: renting a boat, buying the
   town's spell and buying siege weapons each ask "...for N gold?" Yes / No
   first. (Troops already need OK on the keypad.) This is a control, not a
   rule change: the same purchase happens on Yes.
7. **Names instead of numbers on mobile**: dismissing a stack lists the army
   stacks by name and count ("Militia x20"); choosing a continent lists the
   continents by name.
8. **Labels keep their full wording and large size; buttons grow to fit.** On
   mobile the game still draws inside the 320x200 design space, so a sheet
   line holds about 15 large characters. Pairs stack one above the other, and
   a button may take two lines of text (taller buttons, more paging) rather
   than shrinking the text or cutting the words.
9. **No keyboard traces anywhere on mobile**, not only on the new buttons.
   Every line of text a mobile player sees that names a key gets a mobile
   version: pack strings ("Use the arrow keys.", "(space)", "ESC to leave",
   "Ctrl-Q quits; any other key returns", "(Space=Take)", the numbered-picker
   hint, and the rest) and the shell's own defaults. Example: the bridge's
   "Use the arrow keys." becomes "Tap a tile next to you."

## Goals

- Every choice on mobile is a button at least 48 dp tall on a phone in
  landscape, with a label large enough to read at arm's length.
- A child can play from the start through combat, chests, towns, recruiting
  and an audience with the King with fingers alone, without missing taps.
- Desktop is pixel-identical to the pre-2026-11-21 build on every screen,
  except the kept items in decision 2.

## Non-goals

Rule changes; new art or animation; autosave; a portrait layout; a
tutorial; the pack picker (only one pack ships on Android).

## Anti-goals

- Desktop drifting from the original beyond decision 2.
- A mobile choice reachable only by a small target, a key, or Back.
- A mobile tap that does something different from the desktop click or key
  for the same choice (same engine call, same outcome).

## Constraints

- `openbounty-engine` untouched; `backend.rs` stays the only module naming
  macroquad (AGENTS.md).
- Every mobile button resolves to the same code path as today's click or key
  (the click == key rule from `specs/touch-complete-plan.md`).
- New mobile strings use our own words (`tools/check_independent.py`), with a
  `DEFAULT_*` constant beside the code, so a pack may reword them.
- Saves are unchanged and stay compatible between desktop and mobile.

## Approaches

- **A. One choice-sheet component fed by every screen (chosen, my call).**
  Each screen that asks for a choice describes its options as data (title,
  optional body text, buttons: label + the target its tap triggers); one
  mobile renderer draws them as big buttons. Consistent look, one place to
  polish, desktop untouched.
- **B. A hand-made touch layout per screen.** Most freedom per screen, about
  three times the code, and screens drift apart. Rejected.
- **C. A separate mobile front end on the engine.** Duplicates the session
  logic both builds share, which is what keeps the rules identical.
  Rejected.

## The design

### Mobile vs desktop

The session learns which target it runs on (`present::Target::native()`
today decides scaling); views and prompts branch on it only to choose how to
draw and which regions to publish. Desktop code paths draw exactly what the
pre-2026-11-21 build drew, plus the kept items. (How the target reaches the
session and views is left to the builder.) The playtest binary gains a way
to run as mobile so headless tests cover both.

### The choice sheet (my calls, the builder may refine)

- **Shapes**: a **list** (one button per option, one column, or two when
  labels are short), a **pair** (two buttons, stacked: yes/no, the
  chest), and a **keypad** (counts: 0-9, delete, MAX, OK, Back, laid out as
  a 4x4 grid). The name entry uses the keypad's big-key style for its letter
  grid.
- **Size**: buttons at least 26 logical pixels tall (about 50 dp on the
  Pixel 9a at 5.085x; larger on tablets) with a 2-pixel gap; labels drawn at
  double glyph size (16 logical pixels), wrapping to a second line inside a
  taller button when needed (decision 8). Pairs stack vertically.
- **Where**: over the map viewport and bottom panel (the area the bottom
  prompt panel and the views already use), optionally widened over the
  sidebar. A title line on top; body text (e.g. the chest's description, the
  King's line) above the buttons at normal size, wrapped.
- **Paging**: when options do not fit, the sheet shows Prev / Next buttons;
  the Back button is on every sheet and does what Esc does today.
- **Hit regions** are exactly the drawn buttons, published in draw order like
  every other surface.

### Inventory (what gets a sheet on mobile)

| Screen | Sheet | Notes |
|---|---|---|
| Difficulty (after the name) | list, 2 columns | label + days |
| Name entry | big-key letter grid + OK | keeps decision-2's grid on mobile, enlarged |
| Save slot picker (start and Game Menu) | list, paged | slot name, class, days |
| Hero action menu | list, 2 columns | the 17 actions |
| Game Menu | list | Save / Load / Quit |
| Quit confirm | pair | "Quit without saving?" Yes / No |
| Yes/no prompts (search, dismiss last army, surrender, overwrite...) | pair | Yes / No |
| Chest (gold or leadership) | pair | labels from the amounts |
| Dismiss a stack | list | stacks by name and count (decision 7) |
| Choose a continent | list | continents by name (decision 7) |
| Dwelling count | keypad | |
| Town | list | contract, boat (price), spell (name, price), siege (price), info; purchases confirm (decision 6) |
| Home castle | list | Recruit soldiers / Speak with the King |
| Recruit soldiers | list then keypad | troop + cost, then the count |
| Own castle (garrison) | list + mode toggle | the five slots |
| Gate spells | list, paged | destinations |
| Spellbook | list, 2 columns | the 14 spells; cast-mode only shows castable |
| Controls | list | each row cycles its value |
| Combat action menu | list | the 9 actions |
| Combat spell menu | list | the 7 combat spells |
| Combat target pick | a visible Cancel button | tapping a non-target still cancels |
| World map with the orb | a "Whole map" toggle button | |

Already big enough, unchanged on mobile: map tiles (walk, bridge direction),
combat cells, sidebar panels, class figures, message boxes, read-only views
(tap anywhere closes), the win cutscene and game-over taps.

### Wording (decisions 5, 8 and 9)

Mobile labels say what the button does: "Yes", "No", "Back", "Keep 400
gold", "Share it (+40 leadership)", "Rent a boat - 500 gold", "Buy Bridge -
1000 gold". The chest and town labels are built from the amounts the engine
reports. A label longer than one sheet line wraps onto a second line of the
same button (decision 8). The exact copy is the builder's, in our own words,
each with a pack-overridable default.

Mobile text lookup (my call): a mobile build looks a string up first in a
`mobile` mirror of its group (for example `strings.mobile.banners.<key>`),
then falls back to the normal group. The shipped pack gains mobile versions
of every line that names a key (an audit of `game.json` found about 13), and
the shell gains mobile defaults for its own key-naming constants. Desktop
never reads the mirror, so desktop text is unchanged.

### Refused purchases (my call)

The engine checks and buys in one call and has no "would this succeed?"
query, and the engine stays untouched. A mobile town purchase therefore runs
the transaction on a throwaway copy of the game first: if the engine would
refuse (not enough gold, already owned, spellbook full), the refusal shows
straight away with no confirm; otherwise the "...for N gold?" confirm
appears, and Yes runs the real transaction. No rule logic is copied into the
shell.

## Verification

- Headless tests per sheet: every option publishes a region the size of its
  drawn button; tapping it reaches the same outcome as the desktop click or
  key (whole-game snapshot compare, as the existing equivalence tests do).
- A minimum-size test: every mobile tap region is at least 26 logical pixels
  tall.
- Desktop: a keyboard-and-mouse playtest script compared pixel for pixel with
  the pre-2026-11-21 build (commit `ad77d4e`); the only allowed differences
  are decision 2's kept items.
- The click-only playthrough (`shell/tests/touch_only.rs`) runs as mobile.
- On the Pixel 9a and the tablet emulator: play the "done" path from Goals.

## Left to the builder

Exact pixel layout and colours of the sheets; where the target flag lives;
the order of work; the mobile copy (in our own words).
