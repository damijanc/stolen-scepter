# OpenBounty Rust Port

A parity-first Rust port of [OpenBounty](https://github.com/dannyheskett/openbounty),
the MIT-licensed C reimplementation of King's Bounty. `engine/` holds the game
rules and is render-free. `shell/` is the playable macroquad front end.
Contributors and agents should read `AGENTS.md` first.

No original King's Bounty assets live in this repository (see `NOTICES.md`).
The game data comes from a separate checkout of OpenBounty in `reference/`,
which is gitignored.

## Setting up a machine

1. Install a Rust toolchain with [rustup](https://rustup.rs).
2. Clone this repository.
3. Clone OpenBounty into `reference/`, at the commit this port is verified
   against:

   ```sh
   git clone https://github.com/dannyheskett/openbounty.git reference
   git -C reference checkout e674817
   ```

   The stock pack is `reference/assets/kings-bounty/`: `game.json`, maps, art,
   audio and palettes. If you would rather build the pack from your own copy
   of the game, use OpenBounty's extractor (`--extract /path/to/KB.EXE`, see
   its README) and pass the result to `--pack`.

`game-assets/` (the manuals and the original DOS and Amiga releases) is also
gitignored. It is needed only to cross-check behaviour against the manual or
the original binary, not to play. Copy it between machines privately and never
commit it.

## Running

```sh
cargo run -p openbounty-shell --release -- --pack reference/assets/kings-bounty
```

Flags:

| Flag | Meaning |
|---|---|
| `--pack <dir \| file.openbounty \| name>` | The game pack to load. Without it, one discovered pack loads silently and two or more open a picker. |
| `--seed <n>` | Fixed world seed. Without it, the seed comes from time, name and class, so every run is a new world. Use a fixed seed to reproduce a bug. |
| `--save-dir <dir>` | Keep save slots in `<dir>` instead of the user-data directory. Useful for throwaway test saves. |

A reproducible test session:

```sh
cargo run -p openbounty-shell --release -- --seed 1 --save-dir /tmp/kb-play --pack reference/assets/kings-bounty
```

## Playing

- Move with the arrow keys. Diagonals are Home, End, PgUp and PgDn (on a Mac
  laptop, fn plus an arrow), or two arrows pressed together. You can also
  click a revealed tile to walk there.
- `A` army, `V` character, `I` contract, `P` puzzle, `M` world map,
  `O` options reference, `C` controls (sound, music, volume), `U` cast a
  spell, `S` search for the Scepter, `F` fly, `L` land, `W` end the week,
  `D` dismiss a troop.
- ESC closes whatever is open. On the bare overworld it opens the Game Menu
  (Save / Load / Quit). `Q` saves to slot 0 and shows a confirmation, and Ctrl+Q quits.
- F10 opens the debug menu, for setting up scenarios: `G` +50000 gold,
  `V` +100 leadership, `M` spell power, `U` +1 of every spell, `S` siege
  weapons, `F` flight, `N` next continent, `O` clear the fog, `W` win,
  `L` lose screen.
- Rest is numpad 5 only, as in the original, so it cannot be used on a
  keyboard without a numpad.

## Verifying behaviour

`specs/play-verification.md` describes the game section by section, with
scenarios to check in play. Results go into
`specs/play-verification-findings.md`.

## Tests

```sh
cargo fmt --all --check
cargo test --workspace
```
