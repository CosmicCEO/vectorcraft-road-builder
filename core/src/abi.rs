//! VectorCraft plug-in ABI v1 glue (see `docs/plugins.md` in the VectorCraft repository).

use serde_json::{Value, json};

pub fn alloc(size: u32) -> u32 {
    let mut block = Vec::<u8>::with_capacity(size as usize);
    let ptr = block.as_mut_ptr();
    std::mem::forget(block);
    ptr as u32
}

pub fn pack(bytes: &[u8]) -> u64 {
    ((bytes.len() as u64) << 32) | bytes.as_ptr() as u64
}

/// What a plug-in returns: a JSON value, or JSON text it already wrote (cheaper for big outputs).
pub enum Output {
    Json(Value),
    Raw(String),
}

/// Runs `f(input bytes, params)`; an `Err` becomes the plug-in's decline message. Returns `len << 32 | ptr`, or -1/-2 on host errors.
pub fn run(input: &[u8], params: &[u8], f: fn(&[u8], &Value) -> Result<Output, String>) -> i64 {
    let Ok(params) = serde_json::from_slice::<Value>(params) else { return -1 };
    let bytes = match f(input, &params) {
        Ok(Output::Raw(text)) => text.into_bytes(),
        Ok(Output::Json(v)) => match serde_json::to_vec(&v) {
            Ok(b) => b,
            Err(_) => return -2,
        },
        Err(e) => match serde_json::to_vec(&json!({"error": e})) {
            Ok(b) => b,
            Err(_) => return -2,
        },
    };
    pack(bytes.leak()) as i64
}

/// Emits the five exports every plug-in has. `$manifest` is the manifest JSON text; `$run` is `fn(&[u8], &Value) -> Result<Output, String>`.
#[macro_export]
macro_rules! vc_plugin {
    ($manifest:expr, $run:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn vc_abi_version() -> u32 {
            1
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn vc_manifest() -> u64 {
            $crate::abi::pack($manifest.as_bytes())
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn vc_alloc(size: u32) -> u32 {
            $crate::abi::alloc(size)
        }

        /// # Safety
        /// The host guarantees both blocks came from `vc_alloc` and hold `*_len` bytes.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn vc_run(input: *const u8, input_len: u32, params: *const u8, params_len: u32) -> i64 {
            // SAFETY: see the function's contract.
            let (i, p) = unsafe { (std::slice::from_raw_parts(input, input_len as usize), std::slice::from_raw_parts(params, params_len as usize)) };
            $crate::abi::run(i, p, $run)
        }
    };
}
