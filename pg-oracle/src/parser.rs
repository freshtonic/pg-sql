use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::Mutex;

extern "C" {
    fn pgo_raw_parse_check(sql: *const c_char, errmsg: *mut *mut c_char) -> c_int;
    fn pgo_raw_parse_equal(a: *const c_char, b: *const c_char) -> c_int;
    fn pgo_node_to_string(sql: *const c_char) -> *mut c_char;
    fn pgo_free(p: *mut c_void);
}

/// PostgreSQL's parser holds global state; serialise every call.
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq)]
pub enum Equal {
    Equal,
    Differ,
    ErrorLeft,
    ErrorRight,
}

/// True iff PostgreSQL 17.9's raw parser accepts `sql`.
pub fn parse_ok(sql: &str) -> bool {
    let c = CString::new(sql).expect("NUL in SQL");
    let _guard = LOCK.lock().unwrap();
    unsafe { pgo_raw_parse_check(c.as_ptr(), std::ptr::null_mut()) == 0 }
}

/// Compare two SQL strings by PostgreSQL's `equal()` over their raw parse
/// trees (location-insensitive).
pub fn parse_equal(a: &str, b: &str) -> Equal {
    let ca = CString::new(a).expect("NUL in SQL");
    let cb = CString::new(b).expect("NUL in SQL");
    let _guard = LOCK.lock().unwrap();
    match unsafe { pgo_raw_parse_equal(ca.as_ptr(), cb.as_ptr()) } {
        0 => Equal::Equal,
        1 => Equal::Differ,
        2 => Equal::ErrorLeft,
        3 => Equal::ErrorRight,
        n => panic!("unexpected pgo_raw_parse_equal code {n}"),
    }
}

/// `nodeToString` of the raw parse tree, for human-readable failure diffs.
/// `None` if `sql` does not parse.
pub fn node_to_string(sql: &str) -> Option<String> {
    let c = CString::new(sql).expect("NUL in SQL");
    let _guard = LOCK.lock().unwrap();
    unsafe {
        let p = pgo_node_to_string(c.as_ptr());
        if p.is_null() {
            return None;
        }
        let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
        pgo_free(p as *mut c_void);
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_outcomes() {
        assert_eq!(parse_equal("SELECT 1", "SELECT  1"), Equal::Equal);
        assert_eq!(parse_equal("SELECT 1", "SELECT 2"), Equal::Differ);
        assert_eq!(parse_equal("SELECT $$", "SELECT 1"), Equal::ErrorLeft);
    }

    #[test]
    fn parse_check_outcomes() {
        assert!(parse_ok("SELECT 1"));
        assert!(!parse_ok("SELECT FROM FROM"));
    }

    #[test]
    fn over_permissive_numeric_junk_is_rejected_by_pg() {
        // PostgreSQL rejects trailing junk after a numeric literal.
        assert!(!parse_ok("SELECT 123abc"));
    }
}
