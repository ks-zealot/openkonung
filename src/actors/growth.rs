//! Population growth (`FUN_00420A8C`) and the group target/countdown pass
//! (`FUN_0041BD3C`).
//!
//! Growth is **not** a per-frame spawner. The original runs the actual
//! `FUN_00420A8C` creation loop only during `FUN_0043ADF0(mode 0)` (new game /
//! map entry), filling an eligible class-`0x41..0x50` group while its signed
//! `Group+0x02` accumulator is negative and `Group+0x1C < Group+0x18`. Each
//! created member adds the class interval (high 16 bits of `0x45E216 +
//! class*4`) back into the accumulator; the pass then clamps it to `0`. During
//! play `FUN_0041BD3C` slowly raises the target `Group+0x18` and decrements the
//! accumulator, so the next map entry consumes the budget. Save mode `1` never
//! grows (the population cache is restored directly).

use crate::actors::{place_entity, ActorPlacement, DeterministicRng, RuntimeActor};
use crate::population::{Entity, GameTables, Group};

/// `0x45E31E`, indexed by `class - 0x41`: the low 16-bit target interval added
/// when `FUN_0041BD3C` raises `Group+0x18`.
pub const GROWTH_TARGET_INTERVAL: [i16; 16] = [
    0x0E10, 0x1770, 0x2EE0, 0x2328, 0x1770, 0x0E10, 0x1C20, 0x0E10, 0x12C0, 0x1770,
    0x1518, 0x1518, 0x0E10, 0x0E10, 0x12C0, 0x12C0,
];

/// `0x45E216 + class*4` high 16 bits, indexed by `class - 0x41`: the accumulator
/// increment paid per created member at map entry (`FUN_0043ADF0`).
pub const GROWTH_CREATE_INTERVAL: [i16; 16] = [
    0x05A0, 0x0B40, 0x12C0, 0x0E10, 0x0960, 0x0960, 0x0E10, 0x0960, 0x0960, 0x0960,
    0x0708, 0x0960, 0x0780, 0x0E10, 0x0B40, 0x0E10,
];

/// `FUN_0041BD3C` / `FUN_0043ADF0` object-class window. The original compares
/// `class - 0x41 < 0x10` on a signed value; every shipped growth group is in
/// `0x41..=0x50` (the only classes that both carry `0x40` and stay inside the
/// window), so the range is spelled out explicitly here.
pub fn growth_class(class: u8) -> bool {
    (0x41..=0x50).contains(&class)
}

/// `FUN_0041BD3C` growth branch, called every 16 logic frames. Class-`0x41..0x50`
/// groups whose current count is below capacity tick the target countdown; when
/// it underflows the target rises by one and the interval is re-armed. A group
/// still below its target decrements the signed accumulator so the next map
/// entry creates a member.
pub fn update_group_growth(group: &mut Group) {
    if !growth_class(group.class_1e) {
        return;
    }
    if (group.count_18 as i8) < (group.capacity as i8) {
        group.growth_countdown = group.growth_countdown.wrapping_sub(1);
        if group.growth_countdown < 0 {
            group.count_18 = group.count_18.saturating_add(1);
            let interval = GROWTH_TARGET_INTERVAL[(group.class_1e - 0x41) as usize];
            group.growth_countdown = group.growth_countdown.wrapping_add(interval);
        }
    }
    if (group.entity_count as i8) < (group.count_18 as i8) {
        group.growth_timer = group.growth_timer.wrapping_sub(1);
    }
}

/// `FUN_00420A8C` + the `FUN_0043ADF0` creation loop for one group.
///
/// The loop head is the original signed test `Group+0x02 < 0` plus
/// `(char)+0x1C < (char)+0x18`. Each iteration builds one averaged member,
/// places it twice exactly like the original (once inside the constructor, once
/// in the caller), then adds the class interval to the accumulator. A failed
/// placement ends the group's pass; the accumulator is clamped to `0`.
pub fn grow_group(
    tables: &mut GameTables,
    group_index: usize,
    collision: &[u16],
    rng: &mut DeterministicRng,
) -> Vec<RuntimeActor> {
    let Some(group) = tables.groups.get(group_index).cloned() else {
        return Vec::new();
    };
    if !growth_class(group.class_1e) {
        return Vec::new();
    }
    let interval = GROWTH_CREATE_INTERVAL[(group.class_1e - 0x41) as usize] as i32;
    let mut out = Vec::new();
    loop {
        let group = &tables.groups[group_index];
        if group.growth_timer >= 0 || (group.entity_count as i8) >= (group.count_18 as i8) {
            break;
        }
        let new_index = group.first_entity as usize + group.entity_count as usize;
        if new_index >= tables.entities.len() {
            break;
        }
        let Some(mut entity) = average_member(&tables.groups[group_index], &tables.entities, rng)
        else {
            break;
        };
        entity.index = new_index;
        // `FUN_00420A8C` places the new member itself; failure aborts creation.
        let Ok((raw, _attempts)) = place_entity(&tables.groups[group_index], rng, collision) else {
            break;
        };
        entity.x = raw.x as u16;
        entity.y = raw.y as u16;
        tables.entities[new_index] = entity.clone();
        {
            let group = &mut tables.groups[group_index];
            group.entity_count = group.entity_count.saturating_add(1);
        }
        // The caller places the returned member a second time. When that fails
        // the member stays counted but is not registered for this visit.
        let Ok((raw, _attempts)) = place_entity(&tables.groups[group_index], rng, collision) else {
            break;
        };
        if let Some(e) = tables.entities.get_mut(new_index) {
            e.x = raw.x as u16;
            e.y = raw.y as u16;
        }
        entity.x = raw.x as u16;
        entity.y = raw.y as u16;
        {
            let group = &mut tables.groups[group_index];
            group.growth_timer = group.growth_timer.wrapping_add(interval);
        }
        out.push(RuntimeActor {
            entity,
            source_group_id: group_index,
            placement: ActorPlacement::Persisted,
        });
    }
    let group = &mut tables.groups[group_index];
    if group.growth_timer < 0 {
        group.growth_timer = 0;
    }
    out
}

/// Map-entry growth pass (`FUN_0043ADF0` mode 0) over groups 1..200.
///
/// The original initialises a `local_2c` flag to `1`: the **first** group that
/// passes the gate (active object class in `0x41..=0x50`, location match) is
/// skipped and every later eligible group grows. This quirk is reproduced
/// verbatim rather than "fixed".
pub fn grow_map(
    tables: &mut GameTables,
    loc: u8,
    collision: &[u16],
    rng: &mut DeterministicRng,
) -> Vec<RuntimeActor> {
    let mut out = Vec::new();
    let mut first_eligible = true;
    let count = tables.groups.len().min(200);
    for i in 1..count {
        let g = &tables.groups[i];
        if g.loc_id != loc || !growth_class(g.class_1e) {
            continue;
        }
        if first_eligible {
            first_eligible = false;
            continue;
        }
        out.extend(grow_group(tables, i, collision, rng));
    }
    out
}

/// `FUN_00420A8C`: average the existing members' hidden/raw fields, then build
/// one new Entity from a random existing member's visuals.
///
/// Averaged (all truncated after summing every current member, because the
/// constructor is invoked again for each added member):
/// `+0x54` dex base, `+0x58` charisma base, `+0x33` level, `+0x34` money,
/// `+0x4E` base armor, `+0x52` venom, `+0x5E` strength base, `+0x11` raw byte,
/// `+0x62` endurance base. Each average is written to both the base and current
/// half of the pair. HP is fixed at `+0x5C = 0x640`; equipment, inventory and
/// the additional slots are cleared; `+0xD6`/`+0xD8` come from the random
/// member; `kind` is `4` for classes `L`/`F`/`U`, and class `L` also gets
/// `+0x12 = rand % 5 + 1`.
fn average_member(group: &Group, entities: &[Entity], rng: &mut DeterministicRng) -> Option<Entity> {
    let first = group.first_entity as usize;
    let count = group.entity_count as usize;
    if count == 0 || first + count > entities.len() {
        return None;
    }
    let members = &entities[first..first + count];
    let mut dex_base = 0i64;
    let mut charisma_base = 0i64;
    let mut level = 0i64;
    let mut money = 0i64;
    let mut base_armor = 0i64;
    let mut raw_0x52 = 0i64;
    let mut strength_base = 0i64;
    let mut raw_0x11 = 0i64;
    let mut endurance_base = 0i64;
    for e in members {
        let s = e.stats;
        dex_base += s.dexterity_base as i64;
        charisma_base += s.charisma_base as i64;
        level += s.level as i64;
        money += s.money as i64;
        base_armor += s.base_armor as i64;
        raw_0x52 += s.raw_0x52 as i64;
        strength_base += s.strength_base as i64;
        raw_0x11 += s.raw_0x11 as i64;
        endurance_base += s.endurance_base as i64;
    }
    let n = count as i64;
    let source = &members[(rng.next_u32() as usize) % count];
    let mut entity = Entity {
        index: first + count,
        kind: if matches!(group.class_1e, b'L' | b'F' | b'U') {
            4
        } else {
            0
        },
        facing: (rng.next_u32() % 8) as u8,
        flags_a: 0x40,
        flags_b: 0x04,
        archetype: group.class_1e,
        group_id: group.index as u8,
        raw_0x12: 0,
        x: 0,
        y: 0,
        stats: Default::default(),
        equipment: [0; 6],
        inventory: [0; 32],
        additional_equipment: [0; 5],
        special_equipment: 0,
        dialogue_id: 0xFF,
        name_id: 0,
        nickname_id: 0,
        visual_index: source.visual_index,
        palette_offset: source.palette_offset,
    };
    entity.stats.dexterity_base = (dex_base / n) as u16;
    entity.stats.dexterity_current = entity.stats.dexterity_base;
    entity.stats.charisma_base = (charisma_base / n) as u16;
    entity.stats.charisma_current = entity.stats.charisma_base;
    entity.stats.level = (level / n) as u8;
    entity.stats.money = (money / n) as i32;
    entity.stats.base_armor = (base_armor / n) as u16;
    entity.stats.raw_0x52 = (raw_0x52 / n) as u16;
    entity.stats.raw_0x11 = (raw_0x11 / n) as u8;
    entity.stats.strength_base = (strength_base / n) as u16;
    entity.stats.strength_current = entity.stats.strength_base;
    entity.stats.endurance_base = (endurance_base / n) as u16;
    entity.stats.endurance_current = entity.stats.endurance_base;
    entity.stats.hp_internal = 0x640;
    if group.class_1e == b'L' {
        entity.raw_0x12 = (rng.next_u32() % 5) as u8 + 1;
    }
    Some(entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::Rect;

    fn free_collision() -> Vec<u16> {
        vec![0u16; 320 * 160]
    }

    fn group(class: u8) -> Group {
        Group {
            index: 1,
            first_entity: 10,
            rect_a: Rect {
                x1: 1,
                x2: 10,
                y1: 1,
                y2: 10,
            },
            growth_timer: -1,
            growth_countdown: 0,
            count_18: 3,
            capacity: 5,
            loc_id: 1,
            entity_count: 2,
            class_1e: class,
            ..Default::default()
        }
    }

    fn member(index: usize, level: u8, money: i32) -> Entity {
        Entity {
            index,
            kind: 0,
            facing: 0,
            flags_a: 0,
            flags_b: 0,
            archetype: 0x41,
            group_id: 1,
            raw_0x12: 0,
            x: 2,
            y: 2,
            stats: crate::population::EntityStats {
                raw_0x11: 7,
                level,
                money,
                base_armor: 12,
                raw_0x52: 3,
                dexterity_base: 20,
                dexterity_current: 18,
                charisma_base: 14,
                charisma_current: 13,
                strength_base: 16,
                strength_current: 15,
                endurance_base: 10,
                endurance_current: 9,
                ..Default::default()
            },
            equipment: [4; 6],
            inventory: [5; 32],
            additional_equipment: [6; 5],
            special_equipment: 7,
            dialogue_id: 3,
            name_id: 2,
            nickname_id: 1,
            visual_index: 0x22,
            palette_offset: 0x200,
        }
    }

    /// Game.N layout: reserved group 0 at index 0, the fixture group at 1.
    fn tables(g: Group) -> GameTables {
        let mut entities = Vec::new();
        for i in 0..40 {
            entities.push(member(i, 0, 0));
        }
        GameTables {
            groups: vec![Group::default(), g],
            entities,
        }
    }

    #[test]
    fn growth_target_rises_only_for_object_classes() {
        let mut g = group(0x41);
        g.count_18 = 2;
        g.capacity = 3;
        g.growth_countdown = 0;
        update_group_growth(&mut g);
        assert_eq!(g.count_18, 3, "countdown underflow raises the target once");
        assert_eq!(g.growth_countdown, 0x0E10 - 1);
        update_group_growth(&mut g);
        assert_eq!(g.count_18, 3, "target is capped at capacity");
        let mut citizen = group(0x81);
        citizen.count_18 = 2;
        citizen.capacity = 3;
        citizen.growth_countdown = 0;
        update_group_growth(&mut citizen);
        assert_eq!(citizen.count_18, 2);
        assert_eq!(citizen.growth_countdown, 0);
    }

    #[test]
    fn growth_timer_advances_only_below_target() {
        let mut g = group(0x41);
        g.entity_count = 3;
        g.count_18 = 3;
        g.capacity = 3;
        let before = g.growth_timer;
        update_group_growth(&mut g);
        assert_eq!(g.growth_timer, before, "at target: accumulator untouched");
        g.count_18 = 4;
        update_group_growth(&mut g);
        assert_eq!(g.growth_timer, before - 1);
    }

    #[test]
    fn grow_group_consumes_one_interval_and_averages_fields() {
        let mut g = group(0x41);
        g.growth_timer = -1;
        g.count_18 = 3;
        g.entity_count = 2;
        let mut t = tables(g);
        // Member 10 and 11 with distinct authored values.
        t.entities[10].stats.level = 2;
        t.entities[10].stats.money = 100;
        t.entities[11].stats.level = 4;
        t.entities[11].stats.money = 300;
        t.entities[11].visual_index = 0x33;
        let mut rng = DeterministicRng::new(7);
        let created = grow_group(&mut t, 1, &free_collision(), &mut rng);
        assert_eq!(created.len(), 1, "one 1440-step interval absorbs the budget");
        assert_eq!(t.groups[1].entity_count, 3);
        assert_eq!(t.groups[1].growth_timer, 0x05A0 - 1);
        let e = &t.entities[12];
        assert_eq!(e.index, 12);
        assert_eq!(e.archetype, 0x41);
        assert_eq!(e.kind, 0);
        assert_eq!(e.flags_a, 0x40);
        assert_eq!(e.flags_b, 4);
        assert_eq!(e.stats.level, 3);
        assert_eq!(e.stats.money, 200);
        assert_eq!(e.stats.hp_internal, 0x640);
        assert_eq!(e.stats.raw_0x11, 7);
        assert_eq!(e.stats.charisma_base, 14);
        assert_eq!(e.stats.dexterity_base, 20);
        assert_eq!(e.stats.strength_base, 16);
        assert_eq!(e.stats.endurance_base, 10);
        assert_eq!(e.equipment, [0; 6]);
        assert_eq!(e.inventory, [0; 32]);
        assert_eq!(e.dialogue_id, 0xFF);
        assert!(e.x < 320 && e.y < 160, "member was placed");
    }

    #[test]
    fn grow_group_stops_at_target_count() {
        let mut g = group(0x41);
        g.growth_timer = -100_000;
        g.count_18 = 3;
        g.entity_count = 2;
        let mut t = tables(g);
        let mut rng = DeterministicRng::new(11);
        let created = grow_group(&mut t, 1, &free_collision(), &mut rng);
        assert_eq!(created.len(), 1);
        assert_eq!(t.groups[1].entity_count, 3);
        assert_eq!(t.groups[1].growth_timer, 0);
    }

    #[test]
    fn grow_group_requires_negative_accumulator() {
        let mut g = group(0x41);
        g.growth_timer = 0;
        let mut t = tables(g);
        let mut rng = DeterministicRng::new(3);
        assert!(grow_group(&mut t, 1, &free_collision(), &mut rng).is_empty());
        assert_eq!(t.groups[1].entity_count, 2);
    }

    #[test]
    fn grow_group_ignores_citizen_classes() {
        let mut g = group(0x81);
        g.growth_timer = -1;
        let mut t = tables(g);
        let mut rng = DeterministicRng::new(3);
        assert!(grow_group(&mut t, 1, &free_collision(), &mut rng).is_empty());
        assert_eq!(t.groups[1].growth_timer, -1);
    }

    #[test]
    fn grow_group_aborts_when_placement_fails() {
        let mut g = group(0x41);
        g.growth_timer = -1;
        g.entity_count = 2;
        let mut t = tables(g);
        let blocked = vec![0x0001u16; 320 * 160];
        let mut rng = DeterministicRng::new(5);
        assert!(grow_group(&mut t, 1, &blocked, &mut rng).is_empty());
        assert_eq!(t.groups[1].entity_count, 2);
        assert_eq!(t.groups[1].growth_timer, 0, "failed pass clamps the budget");
    }

    #[test]
    fn grow_map_skips_the_first_eligible_group() {
        let mut a = group(0x41);
        a.index = 1;
        a.growth_timer = -1;
        a.entity_count = 2;
        a.count_18 = 3;
        let mut b = group(0x42);
        b.index = 2;
        b.growth_timer = -1;
        b.entity_count = 2;
        b.count_18 = 3;
        b.first_entity = 20;
        let mut t = tables(a);
        t.groups.push(b);
        let mut rng = DeterministicRng::new(9);
        let created = grow_map(&mut t, 1, &free_collision(), &mut rng);
        assert_eq!(created.len(), 1, "only the second eligible group grows");
        assert_eq!(t.groups[1].entity_count, 2, "first eligible group skipped");
        assert_eq!(t.groups[1].growth_timer, -1);
        assert_eq!(t.groups[2].entity_count, 3);
        assert_eq!(created[0].source_group_id, 2);
    }
}

