# Task: signed macOS release on a version tag

You are working in `/Users/damijancavar/development/kings-bounty`, the
repository `github.com/damijanc/stolen-scepter` (public). Read `AGENTS.md`,
`specs/publish-plan.md` (Phase 3 especially) and `tools/README.md` first.

## Rules

- Do NOT commit, push, create tags or releases, or change git state. Leave
  the changes in the working tree for review.
- Do NOT print, echo or log any secret value, not even in a debug step.
- Never touch `reference/` or `game-assets/`.
- Keep the existing comment style: short comments that say why.

## Goal

A release happens only when a `v*` tag is pushed (for example `v0.1.0`). The
workflow then runs, strictly in this order:

1. **Quality**: the existing reusable `.github/workflows/quality.yml` (no
   original content check, `cargo fmt --check`, `cargo test --workspace`).
   Nothing below runs if it fails.
2. **Build and sign**: build the Apple Silicon app, sign it with the
   Developer ID, notarize and staple the app, wrap it in a DMG, sign,
   notarize and staple the DMG, and verify everything.
3. **Publish**: create a GitHub Release for the tag with the DMG attached.

No DMG is built on pushes to `main`, on pull requests, or by hand. Remove the
current `workflow_dispatch` trigger and the "preview" artifact upload from
`macos-dmg.yml`. The "preview" channel may stay in `tools/make_dmg.sh` for
local builds.

## What exists

- `.github/workflows/macos-dmg.yml`: by-hand trigger only. It runs `quality`
  through `needs: quality`, calls `tools/make_dmg.sh "<version>" preview`
  (ad hoc signature), verifies, and uploads the DMG as an artifact. Read it
  in full.
- `tools/make_dmg.sh [VERSION] [CHANNEL]`: builds the release binary,
  assembles `dist/Stolen Scepter.app` (`Info.plist`, icon via
  `tools/gen_icon.py` + `iconutil`, the pack under
  `Contents/Resources/packs/stolen-scepter/`), signs it ad hoc
  (`codesign --force --deep --sign -`), then builds
  `dist/Stolen-Scepter-<VERSION>-<CHANNEL>-macos-arm64.dmg` with an
  Applications link and `LICENSE.txt`. Read it in full.
- `shell/Cargo.toml` version is `0.1.0`.
- A working reference for the same pipeline (a Tauri app, so take the
  signing, notarization, verification and publish steps, not the build) is
  `/Users/damijancavar/development/damaian-client/.github/workflows/macos-dmg.yml`.
  Read it and reuse its secret handling, keychain, notarytool and
  publish-release patterns.

## Secrets (repository secrets, same names as the reference)

`APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY` (for example "Developer ID Application: ... (TEAMID)"),
`APPLE_TEAM_ID`, `APPLE_API_ISSUER`, `APPLE_API_KEY` (key id),
`APPLE_API_KEY_CONTENT` (the .p8 text). The first build step must check that
all seven are set and fail with a clear message naming the missing ones (names
only), before any build work.

## Required design

1. **Signing order matters.** Today the script signs ad hoc and then builds
   the DMG in one go, so a stable app would be inside the DMG unsigned. Split
   it so the workflow can sign between the steps. Either add a mode such as
   `tools/make_dmg.sh VERSION stable --app-only` plus a
   `tools/make_dmg.sh VERSION stable --dmg-only` that packs an existing
   `dist/Stolen Scepter.app`, or have the script sign with
   `$CODESIGN_IDENTITY` when it is set. Your choice; keep `preview` working
   exactly as now for local use, and document the modes in the script header
   and `tools/README.md`.
2. **App signing.** Use a temporary keychain built from `APPLE_CERTIFICATE`,
   as in the reference, deleted in an `if: always()` step. Sign the binary and
   then the bundle with `codesign --force --options runtime --timestamp --sign
   "$APPLE_SIGNING_IDENTITY"`. The game is a plain Rust binary using
   macroquad (OpenGL/Metal window) and kira (CoreAudio output), so it should
   need no entitlements. If you find it does (for example audio input, which
   it does NOT use), add a minimal `tools/entitlements.plist` and explain why.
   Do not use `--deep` for the Developer ID signature: sign the inner binary
   first, then the app.
3. **Notarize the app.** `ditto -c -k --keepParent` the app to a zip, run
   `xcrun notarytool submit --wait` with the API key written to
   `$RUNNER_TEMP` (never into the repo), then `xcrun stapler staple` the app.
   On failure, print `notarytool log <id>` and fail.
4. **DMG.** Build it from the stapled app, sign the DMG with the same
   identity (`--timestamp`), notarize and staple it the same way.
5. **Verify.** `codesign --verify --deep --strict --verbose=2` on the app;
   `spctl --assess --type execute -vv` on the app; `spctl --assess --type open
   --context context:primary-signature -vv` on the DMG;
   `xcrun stapler validate` on both; `hdiutil verify`; and check that
   `Contents/Resources/packs/stolen-scepter/game.json` is in the bundle.
6. **Version.** Take it from the tag (`v0.1.0` gives `0.1.0`) and fail if it
   differs from `shell/Cargo.toml`'s version, so a tag can never ship a build
   that says a different version. The DMG is named
   `Stolen-Scepter-<VERSION>-macos-arm64.dmg` for stable, with no channel
   suffix.
7. **Publish.** A separate `publish-release` job (`needs: build`,
   `permissions: contents: write`) downloads the DMG artifact the build job
   uploaded and creates the release with `gh release create "$TAG" <dmg>
   --title "The Quest for a Stolen Scepter <VERSION>"`. Generate the notes from
   the commit log since the previous `v*` tag, or since the first commit if
   there is none (`git log --pretty='- %s'`; check out with
   `fetch-depth: 0`). A tag containing `-` (for example `v0.2.0-rc.1`) is
   created as a pre-release.
8. **Hygiene.** Use least-privilege `permissions` per job. Keep the
   `concurrency` group, but never cancel a release in progress
   (`cancel-in-progress: false` for tags). Pin actions to the same major
   versions the repo already uses (`actions/checkout@v7`,
   `actions/cache@v6`, `actions/upload-artifact@v7`; use the matching
   `actions/download-artifact`). Don't add third-party actions; the `gh` CLI
   is on the runner.

## Also update

- `specs/publish-plan.md` Phase 3: mark steps 4-6, the secrets check, the tag
  trigger and `publish-release` as done, in its existing style, and add one
  line on how to cut a release (bump `shell/Cargo.toml`, commit, tag
  `vX.Y.Z`, push the tag).
- The header comment of `macos-dmg.yml`, which still says "preview only".
- `tools/README.md` Release section.

## Verify before you stop

- `actionlint` on both workflows if it is installed (`brew list actionlint`);
  otherwise validate the YAML with
  `python3 -c "import yaml,sys; [yaml.safe_load(open(f)) for f in sys.argv[1:]]" .github/workflows/*.yml`
  (install pyyaml into a temporary venv under `$TMPDIR` if needed).
- `bash -n tools/make_dmg.sh` and `shellcheck tools/make_dmg.sh` if
  available.
- Run the local preview path end to end: `tools/make_dmg.sh "" preview` must
  still produce a working ad hoc DMG exactly as before (it needs no sandbox
  for `iconutil` and `hdiutil`). Then remove `dist/`, or check it is
  gitignored.
- If your script change adds an app-only/dmg-only split, run that split
  locally with the ad hoc identity (`-`) to prove the stages chain.
- `python3 tools/check_independent.py` (never pass `--show`).
- `git status` and `git diff --stat`: only the workflow, the script, the docs
  and an optional entitlements file changed.

## Report

The files changed; the final workflow's job graph and the order of its steps;
how the script modes work; the local verification output; and anything you
could not verify locally (the real Developer ID signing, notarization and the
release job only run on GitHub). End with the exact commands the maintainer
runs to cut `v0.1.0`, including pushing `main` first.
