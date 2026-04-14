//! Port of `lib/src/point.c` + inline helpers from `lib/src/point.h`.
//!
//! The C helpers in `point.h` are `static inline` so they remain callable only
//! from C translation units. We re-implement them here as `#[inline]` Rust fns
//! with identical semantics, plus export the one non-inline C entry point
//! (`ts_point_edit`) with `extern "C"` linkage so the linker resolves it
//! against this crate instead of the removed `point.c`.

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TSPoint {
    pub row: u32,
    pub column: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct TSInputEdit {
    pub start_byte: u32,
    pub old_end_byte: u32,
    pub new_end_byte: u32,
    pub start_point: TSPoint,
    pub old_end_point: TSPoint,
    pub new_end_point: TSPoint,
}

#[inline]
#[must_use]
pub const fn point_add(a: TSPoint, b: TSPoint) -> TSPoint {
    if b.row > 0 {
        TSPoint { row: a.row + b.row, column: b.column }
    } else {
        TSPoint { row: a.row, column: a.column + b.column }
    }
}

#[inline]
#[must_use]
pub const fn point_sub(a: TSPoint, b: TSPoint) -> TSPoint {
    if a.row > b.row {
        TSPoint { row: a.row - b.row, column: a.column }
    } else {
        TSPoint {
            row: 0,
            column: if a.column >= b.column { a.column - b.column } else { 0 },
        }
    }
}

/// C ABI entry point — matches `void ts_point_edit(TSPoint*, uint32_t*, const TSInputEdit*)`
/// from `lib/src/point.c`.
///
/// # Safety
/// `point`, `byte`, and `edit` must be valid, non-null, properly aligned
/// pointers to initialized values. The caller retains ownership.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ts_point_edit(
    point: *mut TSPoint,
    byte: *mut u32,
    edit: *const TSInputEdit,
) {
    // Matches C source verbatim (see lib/src/point.c).
    let edit = unsafe { &*edit };
    let mut start_byte = unsafe { *byte };
    let mut start_point = unsafe { *point };

    if start_byte >= edit.old_end_byte {
        start_byte = edit.new_end_byte + (start_byte - edit.old_end_byte);
        start_point = point_add(edit.new_end_point, point_sub(start_point, edit.old_end_point));
    } else if start_byte > edit.start_byte {
        start_byte = edit.new_end_byte;
        start_point = edit.new_end_point;
    }

    unsafe {
        *point = start_point;
        *byte = start_byte;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn p(r: u32, c: u32) -> TSPoint { TSPoint { row: r, column: c } }

    #[test]
    fn point_add_same_row() {
        assert_eq!(point_add(p(3, 5), p(0, 4)), p(3, 9));
    }

    #[test]
    fn point_add_new_row() {
        assert_eq!(point_add(p(3, 5), p(2, 4)), p(5, 4));
    }

    #[test]
    fn point_sub_greater_row_preserves_column() {
        // a.row > b.row: column is taken from a untouched.
        assert_eq!(point_sub(p(3, 9), p(0, 4)), p(3, 9));
        assert_eq!(point_sub(p(5, 4), p(2, 9)), p(3, 4));
    }

    #[test]
    fn point_sub_same_or_lower_row_saturates() {
        // a.row <= b.row: row collapses to 0, column saturates at 0.
        assert_eq!(point_sub(p(3, 9), p(3, 4)), p(0, 5));
        assert_eq!(point_sub(p(2, 4), p(5, 9)), p(0, 0));
    }

    #[test]
    fn edit_before_range_unchanged() {
        let edit = TSInputEdit {
            start_byte: 10, old_end_byte: 20, new_end_byte: 25,
            start_point: p(0, 10), old_end_point: p(0, 20), new_end_point: p(0, 25),
        };
        let mut point = p(0, 5);
        let mut byte = 5u32;
        unsafe { ts_point_edit(&mut point, &mut byte, &edit); }
        assert_eq!(byte, 5);
        assert_eq!(point, p(0, 5));
    }

    #[test]
    fn edit_after_range_shifted() {
        let edit = TSInputEdit {
            start_byte: 10, old_end_byte: 20, new_end_byte: 25,
            start_point: p(0, 10), old_end_point: p(0, 20), new_end_point: p(0, 25),
        };
        let mut point = p(0, 30);
        let mut byte = 30u32;
        unsafe { ts_point_edit(&mut point, &mut byte, &edit); }
        assert_eq!(byte, 35);
        assert_eq!(point, p(0, 35));
    }

    #[test]
    fn edit_inside_range_collapsed_to_new_end() {
        let edit = TSInputEdit {
            start_byte: 10, old_end_byte: 20, new_end_byte: 25,
            start_point: p(0, 10), old_end_point: p(0, 20), new_end_point: p(0, 25),
        };
        let mut point = p(0, 15);
        let mut byte = 15u32;
        unsafe { ts_point_edit(&mut point, &mut byte, &edit); }
        assert_eq!(byte, 25);
        assert_eq!(point, p(0, 25));
    }
}
