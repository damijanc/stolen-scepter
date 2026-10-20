"""Authored three-quarter-view pose skeletons, drawn as OpenPose control images.

Each troop's four combat frames (see TROOP_POSES in gen_ai.py) get a skeleton
built here from joint ANGLES we author -- never from the original art. A
ControlNet OpenPose model then makes SDXL follow the pose and, because the
skeleton's face points +x, the facing: every figure faces right.

Angles are degrees. Limbs: 0 = hanging straight down, +90 = pointing forward
(the way the figure faces), 180 = straight up, -90 = pointing backward. Each
limb takes (upper, lower) ABSOLUTE angles, not relative ones. `lean` tilts the
torso forward from vertical; `look` tilts the face down (+) or up (-).

The body is a THREE-QUARTER view, not a pure profile: turned right, chest
opened toward the viewer, so the shoulders and hips are spread apart and both
eyes show. A pure profile stacks the shoulders and hips on one point, which
the OpenPose ControlNet (trained on poses detected in photos) reads loosely --
the first profile run drew three-quarter bodies anyway and ignored the arms.

Turned right, the figure's RIGHT shoulder and hip sit behind the neck on screen
(left of it) and its LEFT ones ahead of it, so the limbs are named by where
they are on screen: `back_arm` / `back_leg` are OpenPose's R joints,
`front_arm` / `front_leg` its L joints. Attaching each leg to the hip on its
own side keeps a stride from crossing into an X. The right ear shows; the left
one is hidden behind the head.

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
SHOULDER_SPREAD, HIP_SPREAD = 0.055, 0.035   # half-widths, three-quarter view
UPPER_ARM, FOREARM = 0.17, 0.16
THIGH, SHIN = 0.245, 0.245


def _limb(a):
    r = math.radians(a)
    return math.sin(r), -math.cos(r)


def build(lean=0.0, look=0.0, back_arm=(5, 5), front_arm=(-5, -5),
          front_leg=(8, 0), back_leg=(-8, 0), legs=1.0, no_legs=False):
    """18 keypoints (x, y-up) or None for hidden ones, hip centre at origin."""
    pts = [None] * 18
    lr = math.radians(lean)
    up = (math.sin(lr), math.cos(lr))
    fwd = (math.cos(lr), -math.sin(lr))

    def add(p, v, k):
        return (p[0] + v[0] * k, p[1] + v[1] * k)

    neck = add((0.0, 0.0), up, TORSO)
    pts[NECK] = neck
    pts[RSHO] = add(add(neck, up, -NECK_UP), fwd, -SHOULDER_SPREAD)
    pts[LSHO] = add(add(neck, up, -NECK_UP), fwd, SHOULDER_SPREAD)
    for sho, elb, wri, (ua, fa) in ((RSHO, RELB, RWRI, back_arm),
                                    (LSHO, LELB, LWRI, front_arm)):
        pts[elb] = add(pts[sho], _limb(ua), UPPER_ARM)
        pts[wri] = add(pts[elb], _limb(fa), FOREARM)

    # Face turned right: right ear at the back of the head, both eyes ahead of
    # it, the nose furthest right; tilted by `look`.
    head = add(neck, up, 0.085)
    lk = math.radians(look)
    face = (fwd[0] * math.cos(lk) + up[0] * -math.sin(lk),
            fwd[1] * math.cos(lk) + up[1] * -math.sin(lk))
    pts[REAR] = add(head, face, -0.035)
    pts[REYE] = add(add(head, face, 0.005), up, 0.015)
    pts[LEYE] = add(add(head, face, 0.045), up, 0.015)
    pts[NOSE] = add(add(head, face, 0.05), up, -0.015)

    # Hips stay even without legs: OpenPose draws the torso as neck-to-hip.
    pts[RHIP] = add((0.0, 0.0), fwd, -HIP_SPREAD)
    pts[LHIP] = add((0.0, 0.0), fwd, HIP_SPREAD)
    if not no_legs:
        for hip, kne, ank, (ta, sa) in ((RHIP, RKNE, RANK, back_leg),
                                        (LHIP, LKNE, LANK, front_leg)):
            pts[kne] = add(pts[hip], _limb(ta), THIGH * legs)
            pts[ank] = add(pts[kne], _limb(sa), SHIN * legs)
    return pts


# Stances and arm sets, reused across units.
STAND = dict(front_leg=(8, 0), back_leg=(-8, 0))
WIDE = dict(front_leg=(22, 4), back_leg=(-20, -6))
LUNGE = dict(lean=15, front_leg=(45, 5), back_leg=(-35, -25))
CROUCH = dict(front_leg=(60, -15), back_leg=(35, -30))
KNEEL = dict(front_leg=(88, 0), back_leg=(0, -88))
FLY = dict(front_leg=(-60, -85), back_leg=(-70, -95))

SHIELD = (40, 95)          # front arm, shield held out in front


def P(*bases, **kw):
    """A pose: stance dicts merged left to right, then the pose's own values."""
    out = {}
    for b in bases:
        out.update(b)
    out.update(kw)
    return out


POSES = {
    # bow held in the front hand, string drawn with the back hand
    "bow_draw": P(WIDE, front_arm=(90, 90), back_arm=(-95, 80)),
    "bow_loosed": P(WIDE, front_arm=(90, 90), back_arm=(-100, -135)),
    "bow_down": P(WIDE, front_arm=(30, 38), back_arm=(-8, 8)),
    # both hands out front at chest height, bow hand and string hand a hand's
    # width apart: nocked but not drawn (a back hand near the face reads as a
    # full draw, which is what the first version produced)
    "bow_nock": P(WIDE, front_arm=(60, 95), back_arm=(75, 90)),
    # reloading: bow held low in front, the back hand reaching up over the
    # shoulder to the quiver. Replaces the "nocked, not drawn" beat for the
    # archers -- with any SDXL checkpoint a bow held at chest height is drawn
    # as a full draw, so that beat cannot be told apart from frame 0.
    "bow_reach_quiver": P(WIDE, front_arm=(35, 50), back_arm=(165, 215)),
    "bow_forward": P(WIDE, front_arm=(88, 88), back_arm=(-5, 10)),
    # kneeling archer
    "kneel_bow_low": P(KNEEL, front_arm=(40, 60), back_arm=(10, 40)),
    "kneel_bow_raise": P(KNEEL, front_arm=(75, 85), back_arm=(55, 110)),
    "kneel_bow_aim_up": P(KNEEL, front_arm=(125, 125), back_arm=(-70, 115), look=-25),
    "kneel_bow_loosed": P(KNEEL, front_arm=(80, 80), back_arm=(-105, -140)),
    # one-handed weapon in the back hand, shield in the front
    "blade_guard": P(WIDE, back_arm=(60, 90), front_arm=SHIELD),
    "blade_low": P(STAND, back_arm=(0, 20), front_arm=SHIELD),
    "blade_back": P(WIDE, back_arm=(200, 235), front_arm=SHIELD),
    "blade_up": P(WIDE, back_arm=(150, 175), front_arm=SHIELD),
    "blade_overhead": P(WIDE, back_arm=(185, 205), front_arm=SHIELD),
    "blade_strike": P(LUNGE, back_arm=(85, 55), front_arm=SHIELD),
    "blade_thrust": P(LUNGE, back_arm=(85, 90), front_arm=SHIELD),
    "blade_slash_low": P(WIDE, back_arm=(55, 20), front_arm=SHIELD, lean=10),
    "blade_ground": P(WIDE, back_arm=(40, 10), front_arm=SHIELD, lean=25),
    "blade_shoulder": P(STAND, back_arm=(150, 200), front_arm=SHIELD),
    # two-handed polearm
    "pole_low": P(STAND, back_arm=(20, 70), front_arm=(40, 90)),
    "pole_diag": P(STAND, back_arm=(20, 120), front_arm=(45, 135)),
    "pole_level": P(WIDE, back_arm=(40, 90), front_arm=(62, 90)),
    "pole_lunge": P(LUNGE, back_arm=(70, 90), front_arm=(85, 90)),
    # spellcasters
    "cast_out": P(STAND, back_arm=(95, 110), front_arm=(-90, -75)),
    "cast_lowered": P(STAND, back_arm=(65, 75), front_arm=(-65, -75)),
    "cast_up": P(STAND, back_arm=(160, 170), front_arm=(-160, -170), look=-15),
    "cast_hurl": P(LUNGE, back_arm=(90, 90), front_arm=(80, 85)),
    "staff_hold": P(STAND, lean=18, look=10, back_arm=(30, 90), front_arm=(20, 70)),
    "staff_lift": P(STAND, lean=18, look=5, back_arm=(50, 115), front_arm=(25, 75)),
    "staff_lower": P(STAND, lean=20, look=12, back_arm=(20, 65), front_arm=(15, 60)),
    "staff_crackle": P(STAND, lean=12, back_arm=(65, 125), front_arm=(30, 80)),
    # boulder thrower
    "rock_hold": P(STAND, back_arm=(30, 110), front_arm=(22, 105)),
    "rock_windup": P(CROUCH, lean=-8, back_arm=(200, 230), front_arm=(190, 220)),
    "rock_thrown": P(LUNGE, lean=20, back_arm=(100, 95), front_arm=(60, 40)),
    "rock_pickup": P(lean=75, look=40, back_arm=(5, 0), front_arm=(0, -5),
                        front_leg=(25, -10), back_leg=(-5, -15)),
    # bare fists / claws
    "fists_hunch": P(WIDE, lean=15, back_arm=(10, 30), front_arm=(-5, 20)),
    "fists_punch": P(LUNGE, lean=15, back_arm=(90, 90), front_arm=(-20, 30)),
    "fists_back": P(WIDE, lean=12, back_arm=(-40, -10), front_arm=(-30, 0)),
    "fists_tall": P(STAND, back_arm=(5, 5), front_arm=(-5, 0)),
    "reach": P(WIDE, lean=10, back_arm=(90, 90), front_arm=(85, 85)),
    "reach_lurch": P(lean=25, back_arm=(95, 95), front_arm=(85, 80),
                        front_leg=(35, 5), back_leg=(-25, -15)),
    "claw_swipe": P(LUNGE, lean=15, back_arm=(60, 20), front_arm=(100, 110)),
    # cloaked / winged
    "cloak_wrap": P(CROUCH, lean=20, look=10, back_arm=(30, 120), front_arm=(20, 110)),
    "cloak_turn": P(STAND, lean=5, back_arm=(25, 115), front_arm=(15, 105)),
    "cloak_open": P(WIDE, back_arm=(120, 140), front_arm=(-120, -140)),
    "cloak_beam": P(WIDE, back_arm=(90, 90), front_arm=(-120, -140)),
    "wings_up": P(STAND, back_arm=(20, 35), front_arm=(-15, 10)),
    "wings_crouch": P(CROUCH, lean=20, back_arm=(40, 60), front_arm=(20, 45)),
    "wings_spread": P(WIDE, back_arm=(100, 110), front_arm=(-100, -110)),
    "wings_bolt": P(LUNGE, lean=15, back_arm=(90, 90), front_arm=(-60, -40)),
    # floating wraith: no legs
    "float_reach": P(no_legs=True, back_arm=(80, 90), front_arm=(30, 60)),
    "float_drift": P(no_legs=True, lean=15, back_arm=(85, 95), front_arm=(35, 65)),
    "float_rise": P(no_legs=True, back_arm=(130, 150), front_arm=(-130, -150)),
    "float_cast": P(no_legs=True, lean=10, back_arm=(90, 95), front_arm=(-40, -20)),
    # flying fairy: body tipped forward, legs trailing
    "fly_hover": P(FLY, lean=45, back_arm=(60, 80), front_arm=(30, 50)),
    "fly_dip": P(FLY, lean=62, look=15, back_arm=(40, 60), front_arm=(20, 40)),
    "fly_cast": P(FLY, lean=45, back_arm=(95, 95), front_arm=(85, 90)),
    "fly_low": P(FLY, lean=40, back_arm=(30, 50), front_arm=(10, 30)),
}

STOCKY = {"dwarves": 0.72, "gnomes": 0.72}

# Frame order matches TROOP_POSES in gen_ai.py. Units missing here (horses,
# wolves, dragons) are not humanoid; they stay prompt-driven.
UNIT_POSES = {
    "archers": ["bow_draw", "bow_loosed", "bow_down", "bow_reach_quiver"],
    "orcs": ["bow_draw", "bow_loosed", "bow_forward", "bow_reach_quiver"],
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
