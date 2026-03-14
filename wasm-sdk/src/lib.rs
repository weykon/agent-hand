//! agent-hand WASM Guest SDK
//!
//! Types and ABI helpers for building WASM canvas plugins.
//!
//! ## Architecture
//!
//! ```text
//! Host (agent-hand)                Guest (your .wasm module)
//! ──────────────────               ────────────────────────
//! serialize input JSON ──────────► alloc(len) → ptr
//! copy into guest mem  ──────────► [input bytes at ptr]
//! call on_event(ptr,len) ────────► parse input
//!                                  run your logic
//!                                  serialize output
//!                                  store in RESULT buffer
//!                                  return 0 (ok)
//! result_ptr() ──────────────────► return RESULT ptr
//! result_len() ──────────────────► return RESULT len
//! parse output JSON  ◄───────────  [output bytes]
//! apply canvas ops   ◄───────────  done
//! ```
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use agent_hand_wasm_sdk::*;
//!
//! // Declare the ABI exports
//! export_abi!();
//!
//! #[no_mangle]
//! pub extern "C" fn on_event(ptr: i32, len: i32) -> i32 {
//!     let input: PluginInput = match parse_input(ptr, len) {
//!         Ok(v) => v,
//!         Err(_) => return 1,
//!     };
//!
//!     let mut output = PluginOutput::default();
//!
//!     match input.event.as_str() {
//!         "init" => {
//!             output.canvas_ops.push(CanvasOp::add_node("dashboard", "Dashboard", "Process"));
//!         }
//!         "node_click" => {
//!             if let Some(id) = &input.node_id {
//!                 output.log.push(format!("clicked: {}", id));
//!             }
//!         }
//!         _ => {}
//!     }
//!
//!     set_result(&output);
//!     0
//! }
//! ```

pub mod types;

pub use types::*;

use std::cell::RefCell;

// ── Guest Memory Management ─────────────────────────────────────────

thread_local! {
    static RESULT_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

/// Parse input JSON from host-provided memory region.
///
/// # Safety
/// Called with pointers from the host. The host guarantees valid memory.
pub fn parse_input(ptr: i32, len: i32) -> Result<PluginInput, String> {
    let slice = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    serde_json::from_slice(slice).map_err(|e| format!("parse error: {}", e))
}

/// Store output JSON in the result buffer for the host to read.
pub fn set_result(output: &PluginOutput) {
    let bytes = serde_json::to_vec(output).unwrap_or_default();
    RESULT_BUF.with(|buf| {
        *buf.borrow_mut() = bytes;
    });
}

/// Get result buffer pointer (called by host via `result_ptr` export).
pub fn get_result_ptr() -> i32 {
    RESULT_BUF.with(|buf| buf.borrow().as_ptr() as i32)
}

/// Get result buffer length (called by host via `result_len` export).
pub fn get_result_len() -> i32 {
    RESULT_BUF.with(|buf| buf.borrow().len() as i32)
}

/// Macro to export the standard ABI functions.
///
/// Place `export_abi!();` at the top of your guest module's lib.rs.
/// This exports: `alloc`, `result_ptr`, `result_len`.
///
/// You still need to implement `on_event` yourself.
#[macro_export]
macro_rules! export_abi {
    () => {
        #[no_mangle]
        pub extern "C" fn alloc(len: i32) -> i32 {
            let layout = std::alloc::Layout::from_size_align(len as usize, 1).unwrap();
            let ptr = unsafe { std::alloc::alloc(layout) };
            ptr as usize as i32
        }

        #[no_mangle]
        pub extern "C" fn result_ptr() -> i32 {
            $crate::get_result_ptr()
        }

        #[no_mangle]
        pub extern "C" fn result_len() -> i32 {
            $crate::get_result_len()
        }
    };
}
