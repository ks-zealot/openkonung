//! Item hover tooltip tests.

use super::common::*;

/// The detail block of Dragomir's knife carries its name plus the weight and price lines.
#[test]
fn real_item_detail_lines_include_verified_properties() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Ok(game1) = std::fs::read(install_dir().join("Game.1")) else {
        return;
    };
    app.world.res.set_save_game(&game1);
    // Dragomir's starting knife (item instance 1).
    let id = 1u16;
    let Some(info) = app.world.res.item_info(id) else {
        eprintln!("skipping: item 1 not resolvable");
        return;
    };
    assert!(!info.name.is_empty());
    let lines = crate::app::tooltip::item_detail_lines(&app.world.res, id);
    assert!(
        lines.iter().any(|l| l.starts_with("Вес:")),
        "missing weight line: {lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("Цена:") || l.contains("Цена:")),
        "missing price line: {lines:?}"
    );
}

/// Hovering the character-screen equipment rectangle resolves the equipped item id.
#[test]
fn character_hover_resolves_equipment_slot() {
    let Some(mut app) = build_party_app() else {
        eprintln!("skipping: original resources not present");
        return;
    };
    let Ok(game1) = std::fs::read(install_dir().join("Game.1")) else {
        return;
    };
    app.world.res.set_save_game(&game1);
    app.player.equipment = [0, 0, 1, 0, 0, 0];
    app.inventory_owner = crate::inventory::InventoryOwner::Protagonist;
    let (lx, ly) = crate::character_screen::main_base(1, 2);
    app.character_screen = true;
    app.cursor_virtual = ((lx + 35) as f64, (ly + 35) as f64);
    assert_eq!(app.debug_hover_item(), 1);
}
