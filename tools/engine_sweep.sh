#!/bin/sh
# Header-vs-Rust name sweep for the OpenBounty port.
#
# For every function declared in reference/engine/include/*.h, try three
# spellings against engine/src and shell/src as whole words:
#   the C name            GameBuyTroop
#   its snake_case form   game_buy_troop
#   the prefix-stripped   buy_troop   (Game/Map/Fog/resources_/... removed)
# HIT means one spelling appears somewhere. That is a HEURISTIC: a HIT can be a
# comment or an unrelated same-named function, and a MISS can be a core ported
# under a different name. Every MISS needs a hand check (AGENTS.md "Good Next
# Work"). Run from the repository root; needs the reference/ clone.
set -eu

if [ ! -d reference/engine/include ]; then
  echo "engine_sweep: reference/engine/include not found; run from the repo root with the OpenBounty clone present" >&2
  exit 1
fi

for h in reference/engine/include/*.h; do
  grep -hoE '^[A-Za-z_][A-Za-z0-9_ \*]*[ \*]([A-Za-z_][A-Za-z0-9_]*)\(' "$h" \
    | sed -E 's/.*[ \*]([A-Za-z_][A-Za-z0-9_]*)\($/\1/' \
    | while read -r fn; do
        snake=$(printf '%s' "$fn" | sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' | tr 'A-Z' 'a-z')
        short=$(printf '%s' "$snake" | sed -E 's/^(game|map|fog|resources|adventure|flow_apply|spells_adventure|combat|pending|state|save)_//')
        if grep -rqw -e "$fn" -e "$snake" -e "$short" engine/src shell/src; then
          s=HIT
        else
          s=MISS
        fi
        printf '%s\t%s\t%s\t%s\n' "$s" "$(basename "$h")" "$fn" "$short"
      done
done | sort -u
