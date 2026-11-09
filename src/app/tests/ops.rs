//! Quest opcode execution tests (money, items, settlements, save extras).

use super::common::build_party_app;
use super::super::dialogue_ops::QuestContext;
use super::super::*;

const NPC: usize = 539;

fn run(app: &mut PlayableApp, op: u16) -> bool {
    let mut ctx = QuestContext::from_app(app, NPC);
    let result = ctx.exec_op(op);
    ctx.apply_to_app(app);
    result
}

fn ensure_item_table(app: &mut PlayableApp) {
    if app.world.res.save_game.len() >= crate::save::ITEM_TABLE_LEN {
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    if let Ok(game1) = std::fs::read(dir.join("Game.1")) {
        app.world
            .res
            .set_save_game(&game1[..crate::save::ITEM_TABLE_LEN.min(game1.len())]);
    }
}

#[test]
fn money_checks_and_gold_ops() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    app.player.money = 29;
    assert!(!run(&mut app, 0x131)); // > 29
    app.player.money = 30;
    assert!(run(&mut app, 0x131));
    assert!(!run(&mut app, 0x132)); // >= 40
    app.player.money = 40;
    assert!(run(&mut app, 0x132));
    app.player.money = 500;
    assert!(run(&mut app, 0x135));
    app.player.money = 100;
    assert!(!run(&mut app, 0x135));

    app.player.money = 1000;
    assert!(run(&mut app, 0x11D));
    assert_eq!(app.player.money, 995);
    assert!(run(&mut app, 0x129));
    assert_eq!(app.player.money, 1095);

    let xp = app.player.available_xp;
    assert!(run(&mut app, 0x12B));
    assert_eq!(app.player.available_xp, xp + 10);
    assert!(run(&mut app, 0x12C));
    assert_eq!(app.player.available_xp, xp + 30);
    assert!(run(&mut app, 0x12D));
    assert_eq!(app.player.available_xp, xp + 80);
}

#[test]
fn settlement_money_flags_and_reset() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    let Some(si) = app
        .settlements
        .iter()
        .position(|s| s.loc_id as u32 == app.current_map)
    else {
        eprintln!("skipping: no settlement at start map");
        return;
    };
    app.settlements[si].flags_4b2 = 0;
    app.settlements[si].money_pool_10 = 120;
    app.settlements[si].progress_14 = 0xFFFF;
    app.player.money = 200;

    assert!(run(&mut app, 0x137));
    assert!(run(&mut app, 0x11B));
    assert_eq!(app.player.money, 320);
    assert_eq!(app.settlements[si].money_pool_10, 0);
    assert!(!run(&mut app, 0x137));

    app.settlements[si].flags_4b2 = 0x20;
    app.player.money = 200;
    assert!(run(&mut app, 0x11A));
    assert_eq!(app.player.money, 80);
    assert_eq!(app.settlements[si].flags_4b2 & 0x20, 0);

    app.player.money = 0;
    assert!(!run(&mut app, 0x11C));
    assert_eq!(app.player.money, 100);
    assert_eq!(app.settlements[si].flags_4b2 & 0x20, 0x20);

    app.settlements[si].flags_4b2 = 0xFF;
    app.settlements[si].money_pool_10 = 77;
    assert!(run(&mut app, 0x16D));
    assert_eq!(app.settlements[si].flags_4b2 & 0x15, 0);
    assert_eq!(app.settlements[si].money_pool_10, 0);
    assert_eq!(app.settlements[si].progress_14, 0);
}

#[test]
fn item_grant_and_low_family_item_ops() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    ensure_item_table(&mut app);
    app.player.inventory = [0; 32];

    assert!(run(&mut app, 0x124));
    assert!(app.player.inventory.iter().any(|&id| {
        app.world
            .res
            .item_class_subtype(id)
            .map(|(c, s)| (c, s) == (8, 0x12))
            .unwrap_or(false)
    }));

    // Low family 0xA6 -> spawn class 0x0B, subtype N-1.
    assert!(run(&mut app, 0xA6));
    assert!(run(&mut app, 0xDC));
    assert!(run(&mut app, 0x70));
    assert!(!run(&mut app, 0xDC));
}

#[test]
fn quest_item_flag_interface_opcode() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    ensure_item_table(&mut app);
    app.player.inventory = [0; 32];
    app.player.interface_mode_d2 = 0;
    let Some(id) = app
        .world
        .res
        .alloc_item_instance(0x0B, 0x22, 0, 0, 0.0, 0, 0)
    else {
        eprintln!("skipping: no free item instance");
        return;
    };
    app.player.inventory[0] = id;
    assert!(run(&mut app, 0x158));
    assert_eq!(app.player.interface_mode_d2, 0x13);
}

#[test]
fn group_activation_and_actor_predicates() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    let group = app
        .tables
        .as_ref()
        .and_then(|t| t.entities.get(NPC))
        .map(|e| e.group_id as usize)
        .unwrap();
    assert!(run(&mut app, 0x112));
    assert_eq!(
        app.tables
            .as_ref()
            .and_then(|t| t.groups.get(group))
            .map(|g| g.activity_1d)
            .unwrap_or(0),
        1
    );

    app.player.hp_internal = 1600;
    assert!(run(&mut app, 0x14A));
    app.player.charisma_base = 61;
    assert!(run(&mut app, 0x16E));
    app.player.charisma_base = 60;
    assert!(!run(&mut app, 0x16E));
    app.player.interface_mode_d2 = 0x7F;
    assert!(run(&mut app, 0x145));
    app.player.interface_mode_d2 = 0xFF;
    assert!(!run(&mut app, 0x145));
}

#[test]
fn healing_ops_target_the_protagonist_and_party() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    // 0x14E: the healer's skill rises; the leader is restored.
    app.player.hp_internal = 100;
    app.tables.as_mut().unwrap().entities[NPC].stats.healing = 50;
    assert!(run(&mut app, 0x14E));
    assert_eq!(app.player.hp_internal, 1600);
    assert_eq!(
        app.tables.as_ref().unwrap().entities[NPC].stats.healing,
        51,
        "1600-100 missing HP / 1024 + 50"
    );

    // 0x14F: party members are restored, not the settlement group.
    app.recruit_companion(NPC);
    app.player.hp_internal = 100;
    app.party[0].entity.stats.hp_internal = 100;
    app.party[0].entity.stats.healing = 10;
    app.tables.as_mut().unwrap().entities[280].stats.healing = 0;
    assert!(run(&mut app, 0x14F));
    assert_eq!(app.player.hp_internal, 1600);
    assert_eq!(app.party[0].entity.stats.hp_internal, 1600);
    assert!(
        app.party[0].entity.stats.healing > 10,
        "party member healing skill must rise"
    );
    assert_eq!(
        app.tables.as_ref().unwrap().entities[280].stats.healing,
        0,
        "village group must not be healed"
    );
}

#[test]
fn settlement_settle_ops_move_the_npc_and_party_specialist_into_the_village() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    let group_before = app
        .tables
        .as_ref()
        .unwrap()
        .groups[23]
        .entity_count;
    app.recruit_companion(NPC);
    assert_eq!(app.party.len(), 1);
    assert!(run(&mut app, 0x14C));
    assert!(app.party.is_empty());
    let group = &app.tables.as_ref().unwrap().groups[23];
    assert_eq!(group.entity_count, group_before + 1);
    let dest = group.first_entity as usize + group_before as usize;
    assert_eq!(
        app.tables.as_ref().unwrap().entities[dest].group_id,
        23,
        "0x14C settles the NPC into the settlement group"
    );

    // 0x159 settles a party specialist at waypoint 4 (strength/melee slot).
    app.recruit_companion(NPC);
    app.party[0].entity.stats.strength_base = 20;
    app.party[0].entity.stats.melee_mastery = 20;
    let before = app.tables.as_ref().unwrap().groups[23].entity_count;
    assert!(run(&mut app, 0x159));
    assert!(app.party.is_empty());
    let group = &app.tables.as_ref().unwrap().groups[23];
    assert_eq!(group.entity_count, before + 1);
    let dest = group.first_entity as usize + before as usize;
    assert_eq!(app.tables.as_ref().unwrap().entities[dest].group_id, 23);
    assert_eq!(
        app.settlements.iter().find(|s| s.loc_id == 4).unwrap().waypoints[4].owner as usize,
        dest
    );
}

#[test]
fn settlement_requirement_and_timer_writes() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    let Some(si) = app
        .settlements
        .iter()
        .position(|s| s.loc_id as u32 == app.current_map)
    else {
        eprintln!("skipping: no settlement at start map");
        return;
    };
    let group_id = app.settlements[si].group_id as usize;
    if let Some(t) = app.tables.as_mut() {
        if let Some(g) = t.groups.get(group_id) {
            if let Ok(range) = g.entity_range() {
                if let Some(e) = t.entities.get_mut(range.start) {
                    e.stats.building = 10;
                }
            }
        }
    }
    let mut targets: Vec<crate::population::SettlementTarget> = vec![Default::default(); 8];
    targets[1].key = 0;
    targets[1].state = 0;
    targets[1].timer = 0;
    targets[7].key = 0;
    targets[7].state = 1;
    app.settlements[si].targets = targets;
    app.settlements[si].target_base0 = 0;
    app.settlements[si].target_base1 = 1;

    assert!(run(&mut app, 0x151));
    assert!(run(&mut app, 0x163));
    assert_eq!(app.settlements[si].targets[1].timer, 24);
    assert!(!run(&mut app, 0x151));

    app.settlements[si].targets[1].state = 3;
    assert!(run(&mut app, 0x13B));
    assert!(run(&mut app, 0x147));
    assert!(run(&mut app, 0x169));
}

#[test]
fn party_gear_repair_caps_and_refills_durability() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: game data not present");
        return;
    };
    ensure_item_table(&mut app);
    if let Some(tables) = app.tables.as_mut() {
        if let Some(npc) = tables.entities.get_mut(NPC) {
            npc.stats.smithing = 10;
        }
    }
    let Some(over) = app
        .world
        .res
        .alloc_item_instance(2, 0, 0, 0, 200.0, 0, 0)
    else {
        eprintln!("skipping: no free item instance");
        return;
    };
    let Some(under) = app
        .world
        .res
        .alloc_item_instance(2, 0, 0, 0, 50.0, 0, 0)
    else {
        return;
    };
    let Some(quest) = app
        .world
        .res
        .alloc_item_instance(9, 0, 0, 0, 200.0, 0, 0)
    else {
        return;
    };
    app.world.res.set_item_quality(over, 50.0);
    app.world.res.set_item_quality(under, 10.0);
    app.world.res.set_item_quality(quest, 50.0);
    app.player.equipment = [over, under, 0, 0, 0, 0];
    app.player.inventory = [0; 32];
    app.player.inventory[0] = quest;

    assert!(run(&mut app, 0x16A));

    assert_eq!(app.world.res.item_durability(over), Some((170.0, 170.0)));
    assert_eq!(app.world.res.item_durability(under), Some((50.0, 50.0)));
    assert_eq!(app.world.res.item_durability(quest), Some((50.0, 200.0)));
}
