//! Hand-rolled C-ABI WASM interface — no wasm-bindgen, no JS dependencies,
//! matching the project's zero-runtime-deps stance.
//!
//! Protocol: the host calls `aispekt_alloc(len)`, writes UTF-8 JSON
//! (`{"fileName": "...", "content": "...", "repo"?: {...}}`), then calls
//! `aispekt_analyze(ptr, len)`. The return value is a pointer to a buffer
//! whose first 4 bytes are the little-endian byte length of the UTF-8 JSON
//! report that follows. The host copies it out and calls
//! `aispekt_free(ptr)` on the returned pointer.

use aispekt_core::types::AnalysisInput;
use aispekt_core::{analyze, report_to_json};

/// # Safety
/// Host must pass the pointer to `aispekt_free`/`aispekt_analyze` unchanged.
#[no_mangle]
pub extern "C" fn aispekt_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len.max(1));
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr` must come from `aispekt_alloc(len)` or be a buffer returned by
/// `aispekt_analyze` (whose allocation is `4 + payload` bytes, passed as len).
#[no_mangle]
pub unsafe extern "C" fn aispekt_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
    }
}

fn pack_result(payload: String) -> *mut u8 {
    let bytes = payload.into_bytes();
    let mut out = Vec::<u8>::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
    let ptr = out.as_mut_ptr();
    std::mem::forget(out);
    ptr
}

/// # Safety
/// `ptr..ptr+len` must be a valid UTF-8 JSON input buffer written by the host.
///
/// The payload is an envelope — `{"ok": <report>}` or `{"err": "<message>"}`
/// — so success/error discrimination never depends on the report's own key
/// set (a future top-level `error` field in Report must not break the host).
#[no_mangle]
pub unsafe extern "C" fn aispekt_analyze(ptr: *const u8, len: usize) -> *mut u8 {
    let bytes = std::slice::from_raw_parts(ptr, len);
    let payload = match std::str::from_utf8(bytes)
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::from_str::<AnalysisInput>(s).map_err(|e| e.to_string()))
    {
        Ok(input) => format!("{{\"ok\":{}}}", report_to_json(&analyze(&input))),
        Err(e) => serde_json::json!({ "err": e }).to_string(),
    };
    pack_result(payload)
}
