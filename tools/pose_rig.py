"""Authored side-view pose skeletons, drawn as OpenPose control images.

Each troop's four combat frames (see TROOP_POSES in gen_ai.py) get a skeleton
built here from joint ANGLES we author -- never from the original art. A
ControlNet OpenPose model then makes SDXL follow the pose and, because the
skeleton's face points +x, the facing: every figure faces right.

Angles are degrees. Limbs: 0 = hanging straight down, +90 = pointing forward
(the way the figure faces), 180 = straight up, -90 = pointing backward. Each
limb takes (upper, lower) ABSOLUTE angles, not relative ones. `lean` tilts the
torso forward from vertical; `look` tilts the face down (+) or up (-).

"near" is the side toward the viewer. A figure facing right shows its RIGHT
side, so near limbs are OpenPose's R joints and only the right eye/ear show.

Stdlib only.
"""
import math

# COCO-18 keypoint order used by OpenPose control images.
NOSE, NECK, RSHO, RELB, RWRI, LSHO, LELB, LWRI = range(8)
RHIP, RKNE, RANK, LHIP, LKNE, LANK, REYE, LEYE, REAR, LEAR = range(8, 18)

# The standard OpenPose limb list and palette (controlnet_aux draw_bodypose).
LIMBS = [(1, 2), (1, 5), (2, 3), (3, 4), (5, 6), (6, 7), (1, 8), (8, 9), (9, 10),
         (1, 11), (11, 12), (12, 13), (1, 0), (0, 14), (14, 16), (0, 15), (15, 17)]
COLORS = [(255, 0, 0), (255, 85, 0), (255, 170, 0), (255, 255, 0), (170, 255, 0),
          (85, 255, 0), (0, 255, 0), (0, 255, 85), (0, 255, 170), (0, 255, 255),
          (0, 170, 255), (0, 85, 255), (0, 0, 255), (85, 0, 255), (170, 0, 255),
          (255, 0, 255), (255, 0, 170), (255, 0, 85)]

TORSO, NECK_UP = 0.30, 0.02
UPPER_ARM, FOREARM = 0.17, 0.16
THIGH, SHIN = 0.245, 0.245


def _limb(a):
    r = math.radians(a)
    return math.sin(r), -math.cos(r)


def build(lean=0.0, look=0.0, near_arm=(5, 5), far_arm=(-5, -5),
          near_leg=(8, 0), far_leg=(-8, 0), legs=1.0, no_legs=False):
    """18 keypoints (x, y-up) or None for hidden ones, hip centre at origin."""
    pts = [None] * 18
    lr = math.radians(lean)
    up = (math.sin(lr), math.cos(lr))
    fwd = (math.cos(lr), -math.sin(lr))

    def add(p, v, k):
        return (p[0] + v[0] * k, p[1] + v[1] * k)

    neck = add((0.0, 0.0), up, TORSO)
    pts[NECK] = neck
    pts[RSHO] = add(add(neck, up, -NECK_UP), fwd, 0.012)
    pts[LSHO] = add(add(neck, up, -NECK_UP), fwd, -0.02)
    for sho, elb, wri, (ua, fa) in ((RSHO, RELB, RWRI, near_arm),
                                    (LSHO, LELB, LWRI, far_arm)):
        pts[elb] = add(pts[sho], _limb(ua), UPPER_ARM)
        pts[wri] = add(pts[elb], _limb(fa), FOREARM)

    # Face: ear above the neck, eye and nose ahead of it, tilted by `look`.
    ear = add(neck, up, 0.085)
    lk = math.radians(look)
    face = (fwd[0] * math.cos(lk) + up[0] * -math.sin(lk),
            fwd[1] * math.cos(lk) + up[1] * -math.sin(lk))
    pts[REAR] = ear
    pts[REYE] = add(add(ear, face, 0.05), up, 0.012)
    pts[NOSE] = add(add(ear, face, 0.075), up, -0.012)

    # Hips stay even without legs: OpenPose draws the torso as neck-to-hip.
    pts[RHIP] = (0.012, 0.0)
    pts[LHIP] = (-0.02, 0.0)
    if not no_legs:
        for hip, kne, ank, (ta, sa) in ((RHIP, RKNE, RANK, near_leg),
                                        (LHIP, LKNE, LANK, far_leg)):
            pts[kne] = add(pts[hip], _limb(ta), THIGH * legs)
            pts[ank] = add(pts[kne], _limb(sa), SHIN * legs)
    return pts


# Stances and arm sets, reused across units.
STAND = dict(near_leg=(8, 0), far_leg=(-8, 0))
WIDE = dict(near_leg=(22, 4), far_leg=(-20, -6))
LUNGE = dict(lean=15, near_leg=(45, 5), far_leg=(-35, -25))
CROUCH = dict(near_leg=(60, -15), far_leg=(35, -30))
KNEEL = dict(near_leg=(88, 0), far_leg=(0, -88))
FLY = dict(near_leg=(-60, -85), far_leg=(-70, -95))

SHIELD = (40, 95)          # far arm, shield held out in front


def P(*bases, **kw):
    """A pose: stance dicts merged left to right, then the pose's own values."""
    out = {}
    for b in bases:
        out.update(b)
    out.update(kw)
    return out


POSES = {
    # bow held in the far hand, string drawn with the near hand
    "bow_draw": P(WIDE, far_arm=(90, 90), near_arm=(-95, 80)),
    "bow_loosed": P(WIDE, far_arm=(90, 90), near_arm=(-100, -135)),
    "bow_down": P(WIDE, far_arm=(30, 38), near_arm=(-8, 8)),
    "bow_nock": P(WIDE, far_arm=(70, 80), near_arm=(45, 105)),
    "bow_forward": P(WIDE, far_arm=(88, 88), near_arm=(-5, 10)),
    # kneeling archer
    "kneel_bow_low": P(KNEEL, far_arm=(40, 60), near_arm=(10, 40)),
    "kneel_bow_raise": P(KNEEL, far_arm=(75, 85), near_arm=(55, 110)),
    "kneel_bow_aim_up": P(KNEEL, far_arm=(125, 125), near_arm=(-70, 115), look=-25),
    "kneel_bow_loosed": P(KNEEL, far_arm=(80, 80), near_arm=(-105, -140)),
    # one-handed weapon in the near hand, shield in the far
    "blade_guard": P(WIDE, near_arm=(60, 90), far_arm=SHIELD),
    "blade_low": P(STAND, near_arm=(0, 20), far_arm=SHIELD),
    "blade_back": P(WIDE, near_arm=(200, 235), far_arm=SHIELD),
    "blade_up": P(WIDE, near_arm=(150, 175), far_arm=SHIELD),
    "blade_overhead": P(WIDE, near_arm=(185, 205), far_arm=SHIELD),
    "blade_strike": P(LUNGE, near_arm=(85, 55), far_arm=SHIELD),
    "blade_thrust": P(LUNGE, near_arm=(85, 90), far_arm=SHIELD),
    "blade_slash_low": P(WIDE, near_arm=(55, 20), far_arm=SHIELD, lean=10),
    "blade_ground": P(WIDE, near_arm=(40, 10), far_arm=SHIELD, lean=25),
    "blade_shoulder": P(STAND, near_arm=(150, 200), far_arm=SHIELD),
    # two-handed polearm
    "pole_low": P(STAND, near_arm=(20, 70), far_arm=(40, 90)),
    "pole_diag": P(STAND, near_arm=(20, 120), far_arm=(45, 135)),
    "pole_level": P(WIDE, near_arm=(40, 90), far_arm=(62, 90)),
    "pole_lunge": P(LUNGE, near_arm=(70, 90), far_arm=(85, 90)),
    # spellcasters
    "cast_out": P(STAND, near_arm=(95, 110), far_arm=(-90, -75)),
    "cast_lowered": P(STAND, near_arm=(65, 75), far_arm=(-65, -75)),
    "cast_up": P(STAND, near_arm=(160, 170), far_arm=(-160, -170), look=-15),
    "cast_hurl": P(LUNGE, near_arm=(90, 90), far_arm=(80, 85)),
    "staff_hold": P(STAND, lean=18, look=10, near_arm=(30, 90), far_arm=(20, 70)),
    "staff_lift": P(STAND, lean=18, look=5, near_arm=(50, 115), far_arm=(25, 75)),
    "staff_lower": P(STAND, lean=20, look=12, near_arm=(20, 65), far_arm=(15, 60)),
    "staff_crackle": P(STAND, lean=12, near_arm=(65, 125), far_arm=(30, 80)),
    # boulder thrower
    "rock_hold": P(STAND, near_arm=(30, 110), far_arm=(22, 105)),
    "rock_windup": P(CROUCH, lean=-8, near_arm=(200, 230), far_arm=(190, 220)),
    "rock_thrown": P(LUNGE, lean=20, near_arm=(100, 95), far_arm=(60, 40)),
    "rock_pickup": P(lean=75, look=40, near_arm=(5, 0), far_arm=(0, -5),
                        near_leg=(25, -10), far_leg=(-5, -15)),
    # bare fists / claws
    "fists_hunch": P(WIDE, lean=15, near_arm=(10, 30), far_arm=(-5, 20)),
    "fists_punch": P(LUNGE, lean=15, near_arm=(90, 90), far_arm=(-20, 30)),
    "fists_back": P(WIDE, lean=12, near_arm=(-40, -10), far_arm=(-30, 0)),
    "fists_tall": P(STAND, near_arm=(5, 5), far_arm=(-5, 0)),
    "reach": P(WIDE, lean=10, near_arm=(90, 90), far_arm=(85, 85)),
    "reach_lurch": P(lean=25, near_arm=(95, 95), far_arm=(85, 80),
                        near_leg=(35, 5), far_leg=(-25, -15)),
    "claw_swipe": P(LUNGE, lean=15, near_arm=(60, 20), far_arm=(100, 110)),
    # cloaked / winged
    "cloak_wrap": P(CROUCH, lean=20, look=10, near_arm=(30, 120), far_arm=(20, 110)),
    "cloak_turn": P(STAND, lean=5, near_arm=(25, 115), far_arm=(15, 105)),
    "cloak_open": P(WIDE, near_arm=(120, 140), far_arm=(-120, -140)),
    "cloak_beam": P(WIDE, near_arm=(90, 90), far_arm=(-120, -140)),
    "wings_up": P(STAND, near_arm=(20, 35), far_arm=(-15, 10)),
    "wings_crouch": P(CROUCH, lean=20, near_arm=(40, 60), far_arm=(20, 45)),
    "wings_spread": P(WIDE, near_arm=(100, 110), far_arm=(-100, -110)),
    "wings_bolt": P(LUNGE, lean=15, near_arm=(90, 90), far_arm=(-60, -40)),
    # floating wraith: no legs
    "float_reach": P(no_legs=True, near_arm=(80, 90), far_arm=(30, 60)),
    "float_drift": P(no_legs=True, lean=15, near_arm=(85, 95), far_arm=(35, 65)),
    "float_rise": P(no_legs=True, near_arm=(130, 150), far_arm=(-130, -150)),
    "float_cast": P(no_legs=True, lean=10, near_arm=(90, 95), far_arm=(-40, -20)),
    # flying fairy: body tipped forward, legs trailing
    "fly_hover": P(FLY, lean=45, near_arm=(60, 80), far_arm=(30, 50)),
    "fly_dip": P(FLY, lean=62, look=15, near_arm=(40, 60), far_arm=(20, 40)),
    "fly_cast": P(FLY, lean=45, near_arm=(95, 95), far_arm=(85, 90)),
    "fly_low": P(FLY, lean=40, near_arm=(30, 50), far_arm=(10, 30)),
}

STOCKY = {"dwarves": 0.72, "gnomes": 0.72}

# Frame order matches TROOP_POSES in gen_ai.py. Units missing here (horses,
# wolves, dragons) are not humanoid; they stay prompt-driven.
UNIT_POSES = {
    "archers": ["bow_draw", "bow_loosed", "bow_down", "bow_nock"],
    "orcs": ["bow_draw", "bow_loosed", "bow_forward", "bow_nock"],
    "elves": ["kneel_bow_low", "kneel_bow_raise", "kneel_bow_aim_up", "kneel_bow_loosed"],
    "archmages": ["cast_out", "cast_up", "cast_hurl", "cast_up"],
    "barbarians": ["blade_low", "blade_back", "blade_strike", "blade_back"],
    "demons": ["wings_up", "wings_crouch", "wings_spread", "wings_bolt"],
    "druids": ["staff_hold", "staff_lift", "staff_lower", "staff_crackle"],
    "dwarves": ["blade_low", "blade_shoulder", "blade_strike", "blade_back"],
    "ghosts": ["float_reach", "float_drift", "float_rise", "float_cast"],
    "giants": ["rock_hold", "rock_windup", "rock_thrown", "rock_pickup"],
    "gnomes": ["cast_out", "cast_lowered", "cast_up", "cast_up"],
    "knights": ["blade_guard", "blade_up", "blade_overhead", "blade_strike"],
    "militia": ["blade_guard", "blade_back", "blade_thrust", "blade_slash_low"],
    "ogres": ["blade_overhead", "blade_strike", "blade_ground", "blade_shoulder"],
    "peasants": ["pole_diag", "pole_level", "pole_lunge", "pole_level"],
    "pikemen": ["pole_low", "pole_level", "pole_lunge", "pole_level"],
    "skeletons": ["pole_diag", "pole_level", "pole_lunge", "pole_level"],
    "sprites": ["fly_hover", "fly_dip", "fly_cast", "fly_low"],
    "trolls": ["fists_hunch", "fists_punch", "fists_back", "fists_tall"],
    "vampires": ["cloak_wrap", "cloak_turn", "cloak_open", "cloak_beam"],
    "zombies": ["reach", "reach_lurch", "claw_swipe", "reach"],
}


def keypoints(unit, frame):
    """The unit's frame-th skeleton (x, y-up keypoints), or None if unposed."""
    names = UNIT_POSES.get(unit)
    if not names:
        return None
    kw = dict(POSES[names[frame]])
    if unit in STOCKY:
        kw["legs"] = STOCKY[unit]
    return build(**kw)


def _to_canvas(pts, w, h):
    """Scale keypoints into a w x h canvas: a fixed body size, feet low, room ahead."""
    live = [p for p in pts if p]
    xs = [p[0] for p in live]
    ys = [p[1] for p in live]
    scale = 0.62 * h
    bw, bh = (max(xs) - min(xs)) * scale, (max(ys) - min(ys)) * scale
    fit = min(1.0, 0.8 * w / max(bw, 1), 0.8 * h / max(bh, 1))
    scale *= fit
    cx = (min(xs) + max(xs)) / 2
    cy = (min(ys) + max(ys)) / 2
    ox, oy = 0.45 * w, 0.52 * h
    return [None if p is None else (ox + (p[0] - cx) * scale, oy - (p[1] - cy) * scale)
            for p in pts]


def render(pts, w, h):
    """An OpenPose control image (RGBA bytes, black background) of `pts`."""
    px = _to_canvas(pts, w, h)
    img = bytearray(w * h * 4)
    for i in range(w * h):
        img[i * 4 + 3] = 255
    stick = max(3, round(w / 128))

    def paint(x0, y0, x1, y1, r, col):
        lo_x, hi_x = int(max(0, min(x0, x1) - r)), int(min(w - 1, max(x0, x1) + r))
        lo_y, hi_y = int(max(0, min(y0, y1) - r)), int(min(h - 1, max(y0, y1) + r))
        dx, dy = x1 - x0, y1 - y0
        ln = dx * dx + dy * dy or 1.0
        for y in range(lo_y, hi_y + 1):
            for x in range(lo_x, hi_x + 1):
                t = max(0.0, min(1.0, ((x - x0) * dx + (y - y0) * dy) / ln))
                ex, ey = x0 + t * dx - x, y0 + t * dy - y
                if ex * ex + ey * ey <= r * r:
                    o = (y * w + x) * 4
                    img[o:o + 3] = bytes(col)

    # Limbs at 60% like controlnet_aux, far side first; then the joints.
    order = sorted(range(len(LIMBS)),
                   key=lambda k: 0 if any(j in (LSHO, LELB, LWRI, LHIP, LKNE, LANK)
                                          for j in LIMBS[k]) else 1)
    for k in order:
        a, b = LIMBS[k]
        if px[a] and px[b]:
            col = tuple(int(c * 0.6) for c in COLORS[k])
            paint(*px[a], *px[b], stick, col)
    for j, p in enumerate(px):
        if p:
            paint(*p, *p, stick + 1, COLORS[j])
    return bytes(img)
