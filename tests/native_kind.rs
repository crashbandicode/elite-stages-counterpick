use elite_stages_counterpick_core::Fighter;

#[test]
fn descriptor_identity_preserves_forms_classes_and_unknowns() {
    for (kind, key) in [
        (0x00, "mario"),
        (0x24, "squirtle"),
        (0x25, "ivysaur"),
        (0x26, "charizard"),
        (0x48, "mii_brawler"),
        (0x49, "mii_swordfighter"),
        (0x4a, "mii_gunner"),
        (0x4b, "ice_climbers"),
        (0x5a, "pyra"),
        (0x5b, "mythra"),
        (0x5d, "sora"),
    ] {
        assert_eq!(Fighter::from_entry_kind(kind).map(Fighter::key), Some(key));
    }
    for unknown in [0x4c, 0x4d, 0x4e, 0x4f, 0x50, 0x5e, u32::MAX] {
        assert_eq!(Fighter::from_entry_kind(unknown), None);
    }
    let mapped: Vec<_> = (0..=0x5d).filter_map(Fighter::from_entry_kind).collect();
    let unique: std::collections::BTreeSet<_> = mapped.iter().copied().collect();
    assert_eq!(mapped.len(), 89);
    assert_eq!(unique.len(), mapped.len());
    assert!(!mapped.contains(&Fighter::from_key("pokemon_trainer").unwrap()));
    assert!(!mapped.contains(&Fighter::from_key("pyra_and_mythra").unwrap()));
}
