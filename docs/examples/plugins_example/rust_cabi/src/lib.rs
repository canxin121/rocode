use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_void};

use serde_json::Value;

// ---------------------------------------------------------------------------
// C ABI types (v1)
// ---------------------------------------------------------------------------

const ROCODE_CABI_VERSION_V1: u32 = 1;
const ROCODE_CABI_FLAG_THREADSAFE: u32 = 1 << 0;

type CreateFnV1 = extern "C" fn() -> *mut c_void;
type DestroyFnV1 = extern "C" fn(*mut c_void);
type HookCountFnV1 = extern "C" fn(*mut c_void) -> usize;
type HookNameFnV1 = extern "C" fn(*mut c_void, usize) -> *const c_char;
type InvokeHookFnV1 = extern "C" fn(
    *mut c_void,
    *const c_char,
    *const c_char,
    *const c_char,
    *mut i32,
) -> *mut c_char;
type FreeStringFnV1 = extern "C" fn(*mut c_void, *mut c_char);

#[repr(C)]
pub struct RocodePluginDescriptorV1 {
    pub abi_version: u32,
    pub flags: u32,
    pub name: *const c_char,
    pub version: *const c_char,
    pub create: Option<CreateFnV1>,
    pub destroy: Option<DestroyFnV1>,
    pub hook_count: Option<HookCountFnV1>,
    pub hook_name: Option<HookNameFnV1>,
    pub invoke_hook: Option<InvokeHookFnV1>,
    pub free_string: Option<FreeStringFnV1>,
    pub reserved: [usize; 8],
}

// This descriptor is a read-only table of function pointers + string pointers.
// It is safe to share across threads.
unsafe impl Send for RocodePluginDescriptorV1 {}
unsafe impl Sync for RocodePluginDescriptorV1 {}

// ---------------------------------------------------------------------------
// Plugin implementation
// ---------------------------------------------------------------------------

struct Demo;

static NAME: &[u8] = b"cabi-demo\0";
static VERSION: &[u8] = b"0.1.0\0";

static HOOK_CHAT_HEADERS: &[u8] = b"chat.headers\0";
static HOOK_TOOL_DEFINITION: &[u8] = b"tool.definition\0";

extern "C" fn create() -> *mut c_void {
    Box::into_raw(Box::new(Demo)) as *mut c_void
}

extern "C" fn destroy(instance: *mut c_void) {
    if instance.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(instance as *mut Demo));
    }
}

extern "C" fn hook_count(_instance: *mut c_void) -> usize {
    2
}

extern "C" fn hook_name(_instance: *mut c_void, index: usize) -> *const c_char {
    match index {
        0 => HOOK_CHAT_HEADERS.as_ptr() as *const c_char,
        1 => HOOK_TOOL_DEFINITION.as_ptr() as *const c_char,
        _ => std::ptr::null(),
    }
}

fn alloc_string(s: &str) -> *mut c_char {
    CString::new(s)
        .unwrap_or_else(|_| CString::new("<invalid>").unwrap())
        .into_raw()
}

extern "C" fn free_string(_instance: *mut c_void, s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(s));
    }
}

extern "C" fn invoke_hook(
    _instance: *mut c_void,
    hook: *const c_char,
    input_json: *const c_char,
    output_json: *const c_char,
    out_code: *mut i32,
) -> *mut c_char {
    unsafe {
        if !out_code.is_null() {
            *out_code = 0;
        }
    }

    let hook = unsafe { CStr::from_ptr(hook) }.to_str().unwrap_or("");
    let _input = unsafe { CStr::from_ptr(input_json) }
        .to_str()
        .unwrap_or("null");
    let output_raw = unsafe { CStr::from_ptr(output_json) }
        .to_str()
        .unwrap_or("null");

    // Parse the seeded output object.
    let mut output_val: Value = match serde_json::from_str(output_raw) {
        Ok(v) => v,
        Err(e) => {
            unsafe {
                if !out_code.is_null() {
                    *out_code = -32603;
                }
            }
            return alloc_string(&format!("parse output failed: {e}"));
        }
    };

    match hook {
        "chat.headers" => {
            // Inject a header.
            let headers = output_val
                .get_mut("headers")
                .and_then(|v| v.as_object_mut());
            if let Some(h) = headers {
                h.insert(
                    "x-rocode-cabi-demo".to_string(),
                    Value::String("1".to_string()),
                );
            }
        }
        "tool.definition" => {
            // Annotate tool descriptions.
            if let Some(desc) = output_val.get_mut("description") {
                if let Some(s) = desc.as_str() {
                    *desc = Value::String(format!("{s} (cabi-demo)"));
                }
            }
        }
        _ => {
            // Unknown hook: no change.
            return std::ptr::null_mut();
        }
    }

    match serde_json::to_string(&output_val) {
        Ok(s) => alloc_string(&s),
        Err(e) => {
            unsafe {
                if !out_code.is_null() {
                    *out_code = -32603;
                }
            }
            alloc_string(&format!("serialize output failed: {e}"))
        }
    }
}

static DESCRIPTOR: RocodePluginDescriptorV1 = RocodePluginDescriptorV1 {
    abi_version: ROCODE_CABI_VERSION_V1,
    flags: ROCODE_CABI_FLAG_THREADSAFE,
    name: NAME.as_ptr() as *const c_char,
    version: VERSION.as_ptr() as *const c_char,
    create: Some(create),
    destroy: Some(destroy),
    hook_count: Some(hook_count),
    hook_name: Some(hook_name),
    invoke_hook: Some(invoke_hook),
    free_string: Some(free_string),
    reserved: [0; 8],
};

#[no_mangle]
pub extern "C" fn rocode_plugin_descriptor_v1() -> *const RocodePluginDescriptorV1 {
    &DESCRIPTOR
}
