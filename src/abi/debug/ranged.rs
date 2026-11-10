//! Ranged and prop-branch combat diagnostics.
//!
//! Staging helpers for `tools/ranged_smoke.mjs`: they equip a known bow/ammo,
//! place a weak target, issue the original order and read back the projectile
//! and prop-settlement state.

use super::super::*;

/// Retained UTF-8 dump of the last [`debug_ranged_state`] report.
pub static mut DEBUG_RANGED_STATE: Option<Vec<u8>> = None;

/// Diagnostic: equip a short bow plus 30 arrows, put one weak object-family
/// actor a few cells east of the leader and issue the aimed shot.
///
/// Returns `target_entity_id + 1`, or 0 when no game, no object actor or no free
/// firing cell exists.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_ranged_setup() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ranged_test_setup(),
        None => 0,
    }
}

/// Diagnostic: give the staged target enough dexterity that the dodge roll
/// (`rand()%101 <= dex_base*0.5`) always succeeds. Returns 1 on success.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_ranged_make_dodge() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ranged_make_dodge(),
        None => 0,
    }
}

/// Diagnostic: give the staged target the shipped 30-arrow stack so the corpse
/// loot branch produces a lootable arrow pile. Returns 1 on success.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_ranged_target_arrows() -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.ranged_target_arrows(),
        None => 0,
    }
}

/// Diagnostic: stage the prop branch on blocked prop `prop_index`, give the
/// leader plain ammo (`+0x02 == 0`) and issue the prop order.
///
/// The leader is placed on the nearest free cell found by an expanding ring
/// search; `prop_index` selects the `Konung.N` prop record by its run index
/// (`Prop::ri`), so `0` means "record 0", not "first prop in the list".
/// Returns `prop_index + 1` when the order was accepted, else 0.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_prop_setup(prop_index: u32) -> u32 {
    match runtime_mut().game.as_mut() {
        Some(g) => g.prop_test_setup(prop_index as usize),
        None => 0,
    }
}

/// Diagnostic: format the ranged/prop state (live projectiles, ammo slot and
/// quantity, bow durability, attack frame, target HP/death, and the settlement
/// target state/flags) into the shared buffer.
///
/// # Memory contract
///
/// Returns 1 when a game exists and the UTF-8 report was stored (read with
/// [`debug_ranged_state_ptr`]/[`debug_ranged_state_len`]), 0 when no game is
/// running. The report owns its storage; the caller never frees it.
#[no_mangle]
pub extern "C" fn debug_ranged_state() -> u32 {
    match runtime_mut().game.as_deref() {
        Some(g) => {
            unsafe {
                *addr_of_mut!(DEBUG_RANGED_STATE) = Some(g.ranged_debug_report().into_bytes());
            }
            1
        }
        None => 0,
    }
}

/// Start of the last [`debug_ranged_state`] report.
///
/// # Memory contract
///
/// Borrowed UTF-8 owned by the debug buffer; never free it. Null before the
/// first report; read the length from [`debug_ranged_state_len`].
#[no_mangle]
pub extern "C" fn debug_ranged_state_ptr() -> *const u8 {
    match unsafe { &*addr_of!(DEBUG_RANGED_STATE) } {
        Some(v) => v.as_ptr(),
        None => std::ptr::null(),
    }
}

/// Byte length of the last [`debug_ranged_state`] report, or 0.
///
/// # Memory contract
///
/// No pointers cross this boundary; the returned value is the status or
/// quantity described above, never an address.
#[no_mangle]
pub extern "C" fn debug_ranged_state_len() -> u32 {
    match unsafe { &*addr_of!(DEBUG_RANGED_STATE) } {
        Some(v) => v.len() as u32,
        None => 0,
    }
}
