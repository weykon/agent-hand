//! WasmAnalyzer — WASM-based analysis extension (feature-gated).
//!
//! Loads a WASM module via wasmtime, invokes its `analyze` export,
//! and marshals JSON between Rust and the WASM guest.
//!
//! ABI contract:
//!   Guest exports:
//!     - `alloc(len: i32) -> i32`       — allocate `len` bytes, return ptr
//!     - `analyze(ptr: i32, len: i32) -> i32` — analyze JSON at (ptr,len), return 0=ok, 1=err
//!     - `result_ptr() -> i32`          — pointer to result JSON
//!     - `result_len() -> i32`          — length of result JSON
//!
//! Input:  JSON-serialized `CoordinationSlice`
//! Output: JSON-serialized `AnalyzerOutput` (scheduler_hints + memory_candidates)

#![cfg(feature = "wasm")]

use std::path::Path;

use wasmtime::{Engine, Instance, Module, Store};

use super::analyzer::{Analyzer, AnalyzerError, AnalyzerOutput};
use super::hot_brain::CoordinationSlice;

/// A WASM-based analyzer extension.
pub struct WasmAnalyzer {
    id: String,
    version: String,
    engine: Engine,
    module: Module,
}

impl WasmAnalyzer {
    /// Load a WASM module from a file path.
    pub fn from_file(id: &str, version: &str, path: &Path) -> Result<Self, AnalyzerError> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, path)
            .map_err(|e| AnalyzerError::Trap(format!("failed to load WASM module: {}", e)))?;
        Ok(Self {
            id: id.to_string(),
            version: version.to_string(),
            engine,
            module,
        })
    }

    /// Load a WASM module from raw bytes.
    pub fn from_bytes(id: &str, version: &str, bytes: &[u8]) -> Result<Self, AnalyzerError> {
        let engine = Engine::default();
        let module = Module::new(&engine, bytes)
            .map_err(|e| AnalyzerError::Trap(format!("failed to compile WASM module: {}", e)))?;
        Ok(Self {
            id: id.to_string(),
            version: version.to_string(),
            engine,
            module,
        })
    }
}

impl Analyzer for WasmAnalyzer {
    fn id(&self) -> &str {
        &self.id
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn analyze(
        &self,
        slice: &CoordinationSlice,
        _trace_id: &str,
        _ts_ms: u64,
    ) -> Result<AnalyzerOutput, AnalyzerError> {
        // Serialize input
        let input_json = serde_json::to_vec(slice)
            .map_err(|e| AnalyzerError::Serialization(format!("input serialization: {}", e)))?;

        // Create a fresh store + instance per invocation (isolation)
        let mut store = Store::new(&self.engine, ());
        let instance = Instance::new(&mut store, &self.module, &[])
            .map_err(|e| AnalyzerError::Trap(format!("instantiation: {}", e)))?;

        // Get exported functions
        let alloc = instance
            .get_typed_func::<i32, i32>(&mut store, "alloc")
            .map_err(|e| AnalyzerError::Trap(format!("missing 'alloc' export: {}", e)))?;
        let analyze_fn = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "analyze")
            .map_err(|e| AnalyzerError::Trap(format!("missing 'analyze' export: {}", e)))?;
        let result_ptr_fn = instance
            .get_typed_func::<(), i32>(&mut store, "result_ptr")
            .map_err(|e| AnalyzerError::Trap(format!("missing 'result_ptr' export: {}", e)))?;
        let result_len_fn = instance
            .get_typed_func::<(), i32>(&mut store, "result_len")
            .map_err(|e| AnalyzerError::Trap(format!("missing 'result_len' export: {}", e)))?;

        // Get memory export
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| AnalyzerError::Trap("missing 'memory' export".to_string()))?;

        // Allocate space in guest and copy input
        let input_len = input_json.len() as i32;
        let ptr = alloc
            .call(&mut store, input_len)
            .map_err(|e| AnalyzerError::Trap(format!("alloc trap: {}", e)))?;

        memory.data_mut(&mut store)[ptr as usize..ptr as usize + input_json.len()]
            .copy_from_slice(&input_json);

        // Call analyze
        let status = analyze_fn
            .call(&mut store, (ptr, input_len))
            .map_err(|e| AnalyzerError::Trap(format!("analyze trap: {}", e)))?;

        if status != 0 {
            return Err(AnalyzerError::Trap(format!(
                "guest returned error status: {}",
                status
            )));
        }

        // Read result
        let result_ptr = result_ptr_fn
            .call(&mut store, ())
            .map_err(|e| AnalyzerError::Trap(format!("result_ptr trap: {}", e)))?
            as usize;
        let result_len = result_len_fn
            .call(&mut store, ())
            .map_err(|e| AnalyzerError::Trap(format!("result_len trap: {}", e)))?
            as usize;

        let result_bytes = &memory.data(&store)[result_ptr..result_ptr + result_len];
        let output: AnalyzerOutput = serde_json::from_slice(result_bytes)
            .map_err(|e| AnalyzerError::Serialization(format!("output deserialization: {}", e)))?;

        Ok(output)
    }
}
