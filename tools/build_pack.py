"""Build packs/stolen-scepter/game.json from the pack's own source data.

Inputs, all under packs/stolen-scepter/:
  data/rules.json        the game's numbers and neutral ids (no text)
  data/text.json         every name and string the player reads (our writing)
  data/tile_codes.json   the map tile legend   } written by
  data/zones/<id>.json   placements per zone   } tools/gen_maps.py
  maps/<id>.dat          the maps              }

    python3 tools/build_pack.py           # write game.json, then check it
    python3 tools/build_pack.py --check   # check the committed game.json only

The check covers what the engine and shell rely on: every string key the
shell reads exists with its placeholders (STRING_KEYS below), names resolve
(troops, spells, classes, villains, artifacts, castles, towns), and every map
row uses only codes from the tile legend.

Stdlib only.
"""
import argparse
import json
import os
import re
import sys
from collections import deque

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACK = os.path.join(ROOT, "packs", "stolen-scepter")
DATA = os.path.join(PACK, "data")
OUT = os.path.join(PACK, "game.json")
PACK_ID = "stolen-scepter"

# The battle tileset, in the slot order the combat view indexes
# (field, three obstacles, the castle spike and six walls, four cursor frames).
COMBAT_TILES = ["field_grass", "obstacle_01", "obstacle_02", "obstacle_03", "castle_spike"] + \
    [f"castle_wall_{i:02d}" for i in range(1, 7)] + [f"cursor_{i:02d}" for i in range(1, 5)]

# Every string the shell reads, with the placeholders its code substitutes.
# A key missing here makes the shell fall back to its built-in text; a
# placeholder missing makes a value vanish from the screen.
STRING_KEYS = {
    "win": {"header": "", "body": "NAME RANK SCORE", "footer": ""},
    "lose": {"header": "", "body": "NAME RANK SCORE", "footer": ""},
    "banners": {
        "chest_gold": "GOLD LEADERSHIP", "chest_commission": "POINTS",
        "chest_spell_power": "POINTS", "chest_max_spells": "POINTS",
        "chest_new_spell": "COUNT SPELL", "chest_empty": "", "town_header": "NAME",
        "town_gold_label": "GOLD", "town_row_contract": "", "town_row_boat_rent": "COST",
        "town_row_boat_cancel": "", "town_row_info": "", "town_row_spell": "SPELL SPELL_COST",
        "town_row_spell_none": "", "town_row_siege_buy": "SIEGE_COST",
        "town_row_siege_owned": "", "town_contract_new": "REWARD VILLAIN ZONE",
        "town_contract_none": "", "town_boat_vacate_first": "", "town_no_gold": "",
        "town_intel_unavailable": "", "town_intel_castle_under": "NAME",
        "town_intel_owner_rule": "OWNER", "town_intel_owner_none": "",
        "town_intel_owner_player": "", "town_intel_owner_king": "",
        "town_intel_count_named": "LABEL TROOP", "town_intel_count_numeric": "COUNT TROOP",
        "town_intel_monsters_generic": "", "town_intel_no_garrison": "",
        "town_spell_unavailable": "", "town_spell_at_cap": "", "town_spell_can_learn": "LEFT S",
        "town_siege_already": "", "town_siege_purchased": "", "spell_time_stop": "STEPS",
        "spell_find_villain_no_contract": "", "spell_find_villain_success": "CASTLE",
        "spell_find_villain_none": "", "spell_bridge_prompt": "",
        "spell_bridge_built": "COUNT", "spell_bridge_invalid": "",
        "spell_castle_gate_none": "", "spell_castle_gate_choose": "",
        "spell_town_gate_none": "", "spell_town_gate_choose": "",
        "spell_instant_army_fizzle": "", "spell_instant_army_no_room": "",
        "spell_instant_army_success": "QTY TROOP", "spell_raise_control_success": "AMOUNT",
        "spell_gate_teleported": "", "spell_gate_invalid": "",
        "encounter_join_named": "LABEL TROOP", "encounter_join_numeric": "COUNT TROOP",
        "encounter_wanderers": "", "encounter_hostile_header": "",
        "encounter_hostile_unknown": "", "encounter_hostile_count_named": "LABEL TROOP",
        "encounter_hostile_count_numeric": "COUNT TROOP", "alcove_offer": "COST",
        "alcove_already": "", "alcove_taught": "", "alcove_no_gold": "COST",
        "no_spell_banner": "X Y ZONE", "new_game_intro": "CLASS NAME",
        "dwelling_recruit_prompt": "CAP COST COUNT GOLD TROOP",
        "dwelling_none_this_week": "", "dwelling_empty": "", "telecave_teleport": "",
        "telecave_inert": "", "navmap_pickup": "ZONE", "crystal_ball_pickup": "ZONE",
        "astrology_header": "WEEK", "astrology_body": "TROOP", "temp_death": "",
        "combat_give_up_header": "", "combat_give_up_body": "",
        "combat_scouts_header": "", "combat_scouts_count": "COUNT TROOP",
        "combat_scouts_small_band": "", "combat_header_siege": "",
        "combat_header_default": "", "signpost_with_body": "BODY TITLE",
        "signpost_title_only": "TITLE", "budget_header": "WEEK", "budget_on_hand": "",
        "budget_payment": "", "budget_boat": "", "budget_army": "", "budget_balance": "",
        "status_days_left": "DAYS", "status_time_stop": "STEPS", "body_save_confirm": "",
        "body_search": "DAYS", "body_dismiss_pick": "", "body_dismiss_last": "",
        "body_home_castle": "", "body_own_castle": "NAME",
        "body_garrison_row_named": "COUNT INDEX TROOP", "body_garrison_row_empty": "INDEX",
        "body_navigate_row": "INDEX ZONE", "body_no_continents": "",
        "body_must_be_sailing": "", "cannot_garrison_last": "", "no_troop_slots": "",
        "army_cannot_handle": "", "no_troops_to_garrison": "", "castle_garrison_empty": "",
        "spell_unavailable": "", "spell_not_known": "", "spell_unknown": "",
    },
    "combat_log": {
        "melee_hit": "ATK COUNT TGT", "retaliate": "COUNT TGT", "ranged_hit": "ATK COUNT TGT",
        "ranged_no_effect": "ATK TGT", "no_effect_msg": "", "fly": "TROOP", "move": "TROOP",
        "wait": "TROOP", "pass": "TROOP", "frozen": "TROOP", "ooc": "TROOP", "immune": "TROOP",
        "cloned": "COUNT TROOP", "resurrected": "COUNT TROOP", "teleported": "",
        "only_one_spell": "", "no_spell_type": "", "cannot_cast": "", "cast_fireball": "",
        "cast_lightning": "", "cast_turn_undead": "", "select_clone": "", "select_freeze": "",
        "select_resurrect": "", "select_damage": "SPELL", "select_teleport": "",
        "select_dest": "", "cant_shoot": "", "no_ammo": "", "cant_fly": "",
        "give_up_prompt": "", "exit_hint": "",
    },
    "ui": {k: "" for k in (
        "press_esc_to_exit quit_to_dos_prompt out_of_control worldmap_hint_your_map "
        "worldmap_hint_whole_map empty_slot combat_spells_title combat_spells_col_combat "
        "combat_spells_prompt dwelling_kind_plains dwelling_kind_forest dwelling_kind_hill "
        "dwelling_kind_dungeon dwelling_recruit_how_many recruit_soldiers_title "
        "recruit_soldiers_how_many own_castle_mode_garrison own_castle_mode_remove").split()},
    "toasts": {"save_cancelled": "", "save_ok": "", "save_failed": "REASON",
               "load_cancelled": "", "load_ok": "", "load_failed": "REASON", "new_game": ""},
    "contract_view": {"title_no_contract": "", "label_name": "VALUE", "label_alias": "VALUE",
                      "label_reward": "VALUE", "label_last_seen": "VALUE",
                      "label_castle": "VALUE", "alias_none": "", "castle_unknown": "",
                      "features_header": "", "crimes_header": ""},
    "spells_view": {"title": "", "combat_col": "", "adventure_col": ""},
    "stats": {k: "" for k in (
        "leadership commission gold spell_power max_spells villains_caught "
        "artifacts_found castles_garrisoned followers_killed current_score").split()},
    "army_view": {k: "" for k in "skill move morale hit_points damage g_cost".split()},
    "morale": {"normal": "", "low": "", "high": ""},
    "startup": {k: "" for k in (
        "controls_hint class_select_hint class_picker_missing save_picker_title "
        "save_picker_empty save_picker_new_game new_game_table_header "
        "new_game_select_hint").split()},
    "controls": {"title": "", "on": "", "off": ""},
    "prompts": {"text_hint": "", "numeric_range_hint": "COUNT", "yes_no_hint": "",
                "numeric_5_hint": ""},
    "dialog_titles": {k: "" for k in (
        "treasure teleport_cave crystal_ball foes alcove_offer alcove_result "
        "castle_default own_castle search dismiss_army dismiss_last navigate "
        "garrison_pick remove_pick save_confirm lose_fallback win_fallback").split()},
}
MENU_ITEMS = ("back exit save load new_game views options army spells character "
              "contract puzzle view_map").split()


def slug(name):
    return re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")


def load(name):
    with open(os.path.join(DATA, name)) as fh:
        return json.load(fh)


def art(*parts):
    return "art/" + "/".join(parts)


def frames(folder, stem):
    return [art(folder, f"{stem}_{i:02d}.png") for i in range(4)]


def build():
    rules, text = load("rules.json"), load("text.json")
    zones_src = [load(os.path.join("zones", f"{z['id']}.json")) for z in text["zones"]]
    zone_ids = [z["id"] for z in text["zones"]]
    home_index = next(i for i, z in enumerate(rules["zones"]) if z["is_home"])

    # Castles: generated sites, named in zone order; the home zone's first
    # site is the King's Royal Hall (gen_maps puts it nearest the centre).
    castles, zone_castles = [], [[] for _ in zone_ids]
    hc = text["home_castle"]
    for zi, src in enumerate(zones_src):
        names = list(text["castles"][zi])
        sites = list(src["castles"])
        if zi == home_index:
            site = sites.pop(0)
            castle = dict(id=hc["id"], name=hc["name"], x=site["x"], y=site["y"],
                          zone=zone_ids[zi])
            special = dict(rules["home_castle"])
            special["dialog"] = dict(header=hc["dialog_header"], body=hc["dialog_body"])
            special["audience"] = dict(intro=hc["audience_intro"],
                                       rank_up=hc["audience_rank_up"],
                                       more_needed=hc["audience_more_needed"],
                                       final_rank=hc["audience_final_rank"])
            castle["special"] = special
            home = castle
            zone_castles[zi].append(dict(x=site["x"], y=site["y"], id=hc["id"]))
        if len(names) != len(sites):
            raise SystemExit(f"{zone_ids[zi]}: {len(sites)} castle sites, {len(names)} names")
        for site, name in zip(sites, names):
            cid = slug(name)
            castles.append(dict(id=cid, name=name, x=site["x"], y=site["y"],
                                zone=zone_ids[zi], difficulty_tier=zi))
            zone_castles[zi].append(dict(x=site["x"], y=site["y"], id=cid))
    castles.append(home)
    for i, c in enumerate(castles):
        c["index"] = i

    # Towns: each reports on a castle of its own zone, round robin.
    towns, zone_towns = [], [[] for _ in zone_ids]
    for zi, src in enumerate(zones_src):
        names = text["towns"][zi]
        if len(names) != len(src["towns"]):
            raise SystemExit(f"{zone_ids[zi]}: {len(src['towns'])} town sites, {len(names)} names")
        intel = [c["id"] for c in castles if c["zone"] == zone_ids[zi] and "special" not in c]
        for n, (site, name) in enumerate(zip(src["towns"], names)):
            tid = slug(name)
            towns.append(dict(index=len(towns), id=tid, name=name, x=site["x"], y=site["y"],
                              boat=site["boat"], gate=site["gate"], zone=zone_ids[zi],
                              intel_castle=intel[n % len(intel)]))
            zone_towns[zi].append(dict(x=site["x"], y=site["y"], id=tid,
                                       boat_x=site["boat"]["x"], boat_y=site["boat"]["y"]))

    signs_text = text["signs"]
    zones, sign_n = [], 0
    for zi, src in enumerate(zones_src):
        zid = zone_ids[zi]
        zone = dict(id=zid, name=text["zones"][zi]["name"], map=f"maps/{zid}.dat",
                    width=src["width"], height=src["height"],
                    neighbors=[z for z in zone_ids if z != zid],
                    hero_spawn=src["hero_spawn"], is_home=zi == home_index)
        if zi == home_index:
            zone["home_spawn"] = src["home_spawn"]
            zone["magic_alcove"] = src["magic_alcove"]
        zone["salt"] = rules["zones"][zi]["salt"]
        signs = []
        for n, s in enumerate(src["signs"]):
            title, body = signs_text[sign_n % len(signs_text)]
            sign_n += 1
            signs.append(dict(x=s["x"], y=s["y"], id=f"sign_{zid}_{n:02d}", title=title, body=body))
        zone["signs"] = signs
        zone["towns"] = zone_towns[zi]
        zone["castles"] = zone_castles[zi]
        zone["chests"] = [dict(x=c["x"], y=c["y"], id=f"chest_{zid}_{n:03d}")
                          for n, c in enumerate(src["chests"])]
        zone["artifacts"] = []
        zone["dwellings"] = []
        zone["wandering_armies"] = [dict(x=a["x"], y=a["y"], id=f"army_{zid}_{n:03d}")
                                    for n, a in enumerate(src["wandering_armies"])]
        zones.append(zone)

    troops = []
    for i, t in enumerate(rules["troops"]):
        troops.append(dict(index=i, id=t["id"], name=text["troops"][t["id"]],
                           sprite=art("troops", f"{t['id']}_00.png"),
                           **{k: v for k, v in t.items() if k != "id"},
                           anim=frames("troops", t["id"])))
    spells = [dict(index=i, id=s["id"], name=text["spells"][s["id"]], kind=s["kind"],
                   cost=s["cost"]) for i, s in enumerate(rules["spells"])]
    classes = []
    for i, c in enumerate(rules["classes"]):
        t = text["classes"][c["id"]]
        ranks = [dict(id=slug(name), name=name, **r) for name, r in zip(t["ranks"], c["ranks"])]
        classes.append(dict(index=i, id=c["id"], name=t["name"],
                            portrait=art("classes", f"{c['id']}.png"),
                            starting_gold=c["starting_gold"],
                            starting_troops=c["starting_troops"], ranks=ranks))
    villains, descriptions = [], {}
    for i, (v, t) in enumerate(zip(rules["villains"], text["villains"])):
        villains.append(dict(index=i, id=t["id"], name=t["name"], zone=zone_ids[v["zone"]],
                             reward=v["reward"], puzzle_cell=v["puzzle_cell"],
                             portrait=art("villains", f"{t['id']}_00.png"), army=v["army"]))
        descriptions[t["id"]] = dict(alias=t["alias"], features=t["features"],
                                     crimes=t["crimes"])
    artifacts = []
    for i, a in enumerate(rules["artifacts"]):
        t = text["artifacts"][a["id"]]
        artifacts.append(dict(index=i, id=a["id"], name=t["name"], zone=zone_ids[a["zone"]],
                              effect=t["effect"], power=a["power"],
                              icon=art("ui", f"inventory_artifact_{a['id']}.png"),
                              puzzle_cell=a["puzzle_cell"], local_idx=a["local_idx"]))

    strings = dict(text["strings"])
    strings["count_buckets"] = {
        k: [dict(max=m, label=lab) for m, lab in zip(rules["count_bucket_maxes"][k], labels)]
        for k, labels in text["count_buckets"].items()}
    strings["difficulty"] = text["difficulty"]
    strings["keybinds"] = [dict(key=k, label=lab) for k, lab in text["keybinds"]]
    strings["villain_descriptions"] = descriptions

    controls = [dict(c, label=text["controls"][c["id"]]) for c in rules["controls"]]
    pack = dict(
        title=text["title"], version=1, pack_id=PACK_ID, pack_name=text["pack_name"],
        pack_kind="base", render=rules["render"],
        world=dict(rules["world"], starting_zone=zone_ids[home_index],
                   zone_noun=text["zone_noun"], zone_noun_plural=text["zone_noun_plural"],
                   default_name=text["default_name"]),
        time=rules["time"], economy=rules["economy"], tuning=rules["tuning"],
        combat=dict(morale_chart=rules["morale_chart"],
                    number_names=[dict(min=m, label=lab) for m, lab in
                                  zip(rules["number_name_mins"], text["number_names"])]),
        zones=zones, towns=towns, castles=castles, troops=troops, spells=spells,
        classes=classes, villains=villains, artifacts=artifacts,
        tile_codes=load("tile_codes.json"), spawn=rules["spawn"], contract=rules["contract"],
        controls=dict(settings=controls),
        sprites=dict(
            hero=dict(walk=frames("sprites", "hero_walk"), boat=frames("sprites", "boat")),
            ui=dict(puzzle_cover=art("ui", "puzzle_cover.png"),
                    town_backdrop=art("ui", "backdrop_town.png"),
                    castle_backdrop=art("ui", "backdrop_castle.png"),
                    plains_backdrop=art("ui", "backdrop_plains.png"),
                    forest_backdrop=art("ui", "backdrop_forest.png"),
                    hillcave_backdrop=art("ui", "backdrop_hillcave.png"),
                    dungeon_backdrop=art("ui", "backdrop_dungeon.png"),
                    ending_win=art("ui", "end_win_screen.png"),
                    ending_lose=art("ui", "end_lose_screen.png"),
                    view_icons_extra=[art("ui", f"inventory_zone_{z}.png") for z in zone_ids],
                    chrome_overworld=art("ui", "chrome_overworld.png"),
                    splash_logo=art("ui", "splash_logo.png"),
                    splash_title=art("ui", "splash_title.png"),
                    class_picker=art("ui", "class_select_picker.png"),
                    class_highlight=art("ui", "class_select_highlight.png")),
            hud=dict(contract_silhouette=art("ui", "hud_contract_silhouette.png"),
                     siege_silhouette=art("ui", "hud_siege_silhouette.png"),
                     siege_animation=frames("ui", "hud_siege"),
                     magic_silhouette=art("ui", "hud_magic_silhouette.png"),
                     magic_animation=frames("ui", "hud_magic"),
                     puzzle_grid=art("ui", "hud_puzzle_grid.png"),
                     gold_purse=art("ui", "hud_gold_purse.png"),
                     bar_strip=art("ui", "hud_bar_strip.png")),
            combat=[art("combat", f"{name}.png") for name in COMBAT_TILES],
            font=art("font", "font.png"),
            palette="palettes/palette.bin"),
        ending=dict(rules["ending"], grass_tile=art("ui", "end_grass.png"),
                    carpet_tile=art("ui", "end_carpet.png"),
                    hero_tile=art("ui", "end_hero.png"),
                    throne_backdrop=art("ui", "end_lose_screen.png")),
        colors=rules["colors"],
        credits=dict(image=art("ui", "class_select_highlight.png"), **text["credits"]),
        audio=dict(tracks=dict(openworld="audio/openworld.ogg", combat="audio/combat.ogg"),
                   tunes=dict(walk="audio/tune_walk.wav", bump="audio/tune_bump.wav",
                              chest="audio/tune_chest.wav", defeat="audio/tune_defeat.wav")),
        strings=strings,
    )
    return pack


def land_components(rows, codes, blocked):
    """8-connected grass/desert regions the hero can walk, minus `blocked`."""
    height, width = len(rows), len(rows[0])

    def walkable(x, y):
        if (x, y) in blocked:
            return False
        code = codes.get(rows[y][x], {})
        return not code.get("blocks_foot") and code.get("terrain") in ("grass", "desert")

    seen, out = set(), []
    for y in range(height):
        for x in range(width):
            if (x, y) in seen or not walkable(x, y):
                continue
            comp, q = [], deque([(x, y)])
            seen.add((x, y))
            while q:
                px, py = q.popleft()
                comp.append((px, py))
                for dx in (-1, 0, 1):
                    for dy in (-1, 0, 1):
                        n = (px + dx, py + dy)
                        if 0 <= n[0] < width and 0 <= n[1] < height \
                                and n not in seen and walkable(*n):
                            seen.add(n)
                            q.append(n)
            out.append(comp)
    return out


def check(pack):
    problems = []
    strings = pack["strings"]
    for section, keys in STRING_KEYS.items():
        have = strings.get(section, {})
        for key, wanted in keys.items():
            if key not in have:
                problems.append(f"strings.{section}.{key}: missing")
                continue
            got = set(re.findall(r"%([A-Z_]+)%", have[key]))
            if got != set(wanted.split()):
                problems.append(f"strings.{section}.{key}: placeholders {sorted(got)}, "
                                f"expected {sorted(wanted.split())}")
    items = strings.get("menu", {}).get("items", {})
    problems += [f"strings.menu.items.{k}: missing" for k in MENU_ITEMS if k not in items]
    troop_ids = {t["id"] for t in pack["troops"]}
    for v in pack["villains"]:
        problems += [f"villain {v['id']}: unknown troop {a['troop']}"
                     for a in v["army"] if a["troop"] not in troop_ids]
    for c in pack["classes"]:
        problems += [f"class {c['id']}: unknown troop {t['id']}"
                     for t in c["starting_troops"] if t["id"] not in troop_ids]
    castle_ids = {c["id"] for c in pack["castles"]}
    problems += [f"town {t['id']}: unknown intel castle" for t in pack["towns"]
                 if t["intel_castle"] not in castle_ids]
    if len(castle_ids) != len(pack["castles"]):
        problems.append("castle ids are not unique")
    if len({t["id"] for t in pack["towns"]}) != len(pack["towns"]):
        problems.append("town ids are not unique")
    if sum(1 for c in pack["castles"] if c.get("special", {}).get("flow") == "audience") != 1:
        problems.append("there must be exactly one audience (home) castle")
    codes = set(pack["tile_codes"])
    for z in pack["zones"]:
        with open(os.path.join(PACK, z["map"])) as fh:
            rows = [r for r in fh.read().split("\n") if r]
        if len(rows) != z["height"] or any(len(r) != z["width"] for r in rows):
            problems.append(f"{z['map']}: not {z['width']}x{z['height']}")
            continue
        bad = {ch for r in rows for ch in r if ch not in codes}
        if bad:
            problems.append(f"{z['map']}: codes not in the legend: {sorted(bad)}")
            continue
        # A castle's walls block the hero and its gate bounces them back, so a
        # site in a one-tile pass would seal the land behind it. Every walkable
        # region must survive the footprints whole.
        walls = {(c["x"] + dx, c["y"] + dy) for c in z["castles"]
                 for dx, dy in ((-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0))}
        owner = {tile: i for i, comp in enumerate(land_components(rows, pack["tile_codes"], walls))
                 for tile in comp}
        for comp in land_components(rows, pack["tile_codes"], set()):
            if len({owner[t] for t in comp if t not in walls}) > 1:
                problems.append(f"{z['id']}: castle walls sever the walkable land")
                break
    return problems


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="check the existing game.json only")
    args = ap.parse_args()
    if args.check:
        with open(OUT) as fh:
            pack = json.load(fh)
    else:
        pack = build()
        with open(OUT, "w") as fh:
            json.dump(pack, fh, indent=1, ensure_ascii=False)
            fh.write("\n")
    problems = check(pack)
    for p in problems:
        print(p)
    print(f"{os.path.relpath(OUT, ROOT)}: {len(pack['zones'])} zones, {len(pack['towns'])} "
          f"towns, {len(pack['castles'])} castles, {len(pack['villains'])} villains; "
          f"{len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
