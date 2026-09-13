use openbounty_engine::combat::combat_test_digest;

const CASES: &[(&str, u64)] = &[
    ("12345:peasants:50:orcs:30:5", 0xa25e_1141_f58e_75d3),
    ("99:knights:10:dragons:5:3", 0xb7c1_e985_8f45_7aaf),
    ("777:demons:20:peasants:100:4", 0x0fb4_9e22_fa60_e35c),
    ("42:archers:25:wolves:20:6", 0x2658_21b6_8355_69f8),
    ("1:dragons:5:peasants:100:1", 0x5e51_c26b_4909_3a9c),
    ("1:peasants:50:peasants:50:5", 0xbcf3_82e1_6c5a_05c4),
    ("1:archers:20:archers:20:5", 0x3fd3_d372_e8f3_254c),
    ("1:pikemen:15:pikemen:15:5", 0x6c6d_fc8c_c97d_3706),
    ("1:knights:8:knights:8:5", 0x14f9_c776_20c7_85ec),
    ("1:dragons:3:dragons:3:3", 0xe2be_84f3_c28d_d790),
    ("42:archers:20:peasants:100:4", 0x9fd0_0c86_26b2_b9f2),
    ("42:knights:10:orcs:20:5", 0x6a33_d496_cd28_4eb9),
    ("42:sprites:30:ogres:5:4", 0x4aa3_f46c_732c_1791),
    ("42:wolves:20:militia:40:4", 0x0b91_a355_7756_238e),
    ("42:ghosts:10:skeletons:20:5", 0x1b2d_1c47_1987_3d4e),
    ("7:dragons:1:peasants:200:3", 0x9f8d_7de4_dc70_890d),
    ("7:peasants:1:peasants:1:2", 0x4f23_ac9f_a083_9fbd),
    ("7:trolls:50:trolls:50:8", 0xb8d2_d54f_ddde_fb59),
    ("100:archers:25:orcs:15:5", 0x0779_2e31_2e0a_38a5),
    ("200:archers:25:orcs:15:5", 0xbbdf_349c_ae47_9291),
    ("300:archers:25:orcs:15:5", 0x288a_8154_ba98_16a4),
    ("400:archers:25:orcs:15:5", 0xa8c9_90b6_2a50_a7c0),
    ("500:archers:25:orcs:15:5", 0x80ca_ddaf_7e21_a98e),
    ("13:vampires:10:zombies:30:5", 0x73f3_eaba_d936_26e6),
    ("21:archmages:5:demons:8:4", 0x0da6_c5a8_1262_affe),
];

#[test]
fn openbounty_combat_digest_goldens() {
    for (spec, expected) in CASES {
        let (seed, attacker, attacker_count, defender, defender_count, rounds) = parse_spec(spec);
        let got = combat_test_digest(
            seed,
            attacker,
            attacker_count,
            defender,
            defender_count,
            rounds,
        );
        assert_eq!(got, *expected, "spec {spec}");
    }
}

fn parse_spec(spec: &str) -> (u64, &str, i32, &str, i32, i32) {
    let mut parts = spec.split(':');
    let seed = parts.next().unwrap().parse().unwrap();
    let attacker = parts.next().unwrap();
    let attacker_count = parts.next().unwrap().parse().unwrap();
    let defender = parts.next().unwrap();
    let defender_count = parts.next().unwrap().parse().unwrap();
    let rounds = parts.next().unwrap().parse().unwrap();
    assert!(parts.next().is_none(), "malformed spec {spec}");
    (
        seed,
        attacker,
        attacker_count,
        defender,
        defender_count,
        rounds,
    )
}
