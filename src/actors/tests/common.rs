//! Shared fixtures for [crate::actors] tests.

use super::super::*;

use crate::population::{Entity, EntityStats, Group};

/// the production wander path relies on `wander_target`'s strict-interior rule.
pub(super) fn rect_contains(rect: &Rect, c: (i32, i32)) -> bool {
    let (x1, x2) = (rect.x1 as i32, rect.x2 as i32);
    let (y1, y2) = (rect.y1 as i32, rect.y2 as i32);
    c.0 >= x1 && c.0 <= x2 && c.1 >= y1 && c.1 <= y2
}

pub(super) fn group(
    index: usize,
    first: u16,
    count: u8,
    loc: u8,
    class: u8,
    rect_a: Rect,
) -> Group {
    Group {
        index,
        first_entity: first,
        rect_a,
        rect_b: Rect::default(),
        count_18: count,
        capacity: count,
        loc_id: loc,
        entity_count: count,
        class_1e: class,
        activity_1d: 0,
        visited: false,
    }
}

pub(super) fn entity(index: usize, kind: u8, archetype: u8, x: u16, y: u16) -> Entity {
    Entity {
        index,
        kind,
        facing: 3,
        flags_a: 0,
        flags_b: 0,
        archetype,
        group_id: 0,
        raw_0x12: 0,
        x,
        y,
        stats: EntityStats::default(),
        equipment: [0; 6],
        inventory: [0; 32],
        dialogue_id: 255,
        name_id: 0,
        nickname_id: 0,
        visual_index: 0,
        palette_offset: 0,
        additional_equipment: [0; 5],
        special_equipment: 0,
    }
}

pub(super) fn tables(groups: Vec<Group>, entities: Vec<Entity>) -> GameTables {
    GameTables { groups, entities }
}

pub(super) fn free_collision() -> Vec<u16> {
    vec![0u16; 320 * 160]
}
