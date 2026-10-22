//! Unit tests for the [`crate::names`] module.

use super::*;

fn konung() -> Option<Vec<u8>> {
    std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../konung.res")).ok()
}

#[test]
fn character_name_table_reference_values() {
    let Some(d) = konung() else {
        eprintln!("skipping: konung.res not present");
        return;
    };
    assert_eq!(character_name(&d, 124).as_deref(), Some("Драгомир"));
    assert_eq!(character_name(&d, 151).as_deref(), Some("Михаил"));
    assert_eq!(character_name(&d, 10).as_deref(), Some("Волк"));
    assert_eq!(character_name(&d, 0), None);
}

#[test]
fn class_name_table_reference_values() {
    let Some(d) = konung() else { return };
    assert_eq!(class_name(&d, 1).as_deref(), Some("Храбрец"));
    assert_eq!(class_name(&d, 2).as_deref(), Some("Купец"));
    assert_eq!(class_name(&d, 3).as_deref(), Some("Охотник"));
    assert_eq!(class_name(&d, 4).as_deref(), Some("Вождь"));
    assert_eq!(class_name(&d, 5).as_deref(), Some("Воин"));
    assert_eq!(class_name(&d, 0), None);
}
