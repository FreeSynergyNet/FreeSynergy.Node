// Cursor navigation helpers — Single Source of Truth for list cursor movement.
//
// Design Pattern: Utility Library — three free functions used everywhere a
// list has a cursor (settings_cursor, lang_cursor, sidebar, overlays).
// Replace manual `if cursor > 0 { cursor -= 1 }` patterns throughout the code.
//
// Usage:
//   cursor::up(&mut state.settings_cursor);
//   cursor::down(&mut state.settings_cursor, stores.len());
//   cursor::clamp(&mut state.lang_cursor, total);

/// Move cursor up by one, clamping to 0.
#[inline]
pub fn up(cursor: &mut usize) {
    if *cursor > 0 { *cursor -= 1; }
}

/// Move cursor down by one, clamping to len-1.
#[inline]
pub fn down(cursor: &mut usize, len: usize) {
    if *cursor + 1 < len { *cursor += 1; }
}

/// Clamp cursor so it stays within [0, len-1].
/// Call after deleting items to avoid out-of-bounds.
#[inline]
pub fn clamp(cursor: &mut usize, len: usize) {
    if len == 0 { *cursor = 0; } else { *cursor = (*cursor).min(len - 1); }
}
