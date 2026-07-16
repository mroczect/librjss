use libloading::{Library, Symbol};
use std::path::PathBuf;

fn shared_lib_path() -> PathBuf {
    let mut base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("target")
        .join("release");  // atau "debug" jika build debug

    let lib_name = if cfg!(target_os = "windows") {
        "librjss.dll"
    } else if cfg!(target_os = "macos") {
        "liblibrjss.dylib"
    } else {
        "liblibrjss.so"
    };
    base.push(lib_name);
    base
}

#[test]
fn test_load_library_and_call_trivial_function() {
    let path = shared_lib_path();
    assert!(path.exists(), "Shared library not found at {:?}. Run `cargo build --release` first.", path);

    unsafe {
        let lib = Library::new(&path).expect("Failed to load shared library");

        // --- Test rjss_config_new_default ---
        type ConfigNew = unsafe extern "C" fn() -> *mut std::ffi::c_void;
        let config_new: Symbol<ConfigNew> = lib.get(b"rjss_config_new_default").unwrap();
        let config_ptr = config_new();
        assert!(!config_ptr.is_null());

        // --- Test rjss_config_free ---
        type ConfigFree = unsafe extern "C" fn(*mut std::ffi::c_void);
        let config_free: Symbol<ConfigFree> = lib.get(b"rjss_config_free").unwrap();
        config_free(config_ptr); // hanya memastikan tidak crash

        // --- Test rjss_error_message ---
        type ErrorMessage = unsafe extern "C" fn(i32) -> *mut std::ffi::c_char;
        let error_msg: Symbol<ErrorMessage> = lib.get(b"rjss_error_message").unwrap();
        let msg_ptr = error_msg(2); // InvalidCredentials
        assert!(!msg_ptr.is_null());
        let c_str = std::ffi::CStr::from_ptr(msg_ptr);
        let msg = c_str.to_str().unwrap();
        assert!(msg.contains("invalid credentials"));

        type FreeString = unsafe extern "C" fn(*mut std::ffi::c_char);
        let free_string: Symbol<FreeString> = lib.get(b"rjss_free_string").unwrap();
        free_string(msg_ptr);

        println!("✅ Basic FFI functions work correctly");
    }
}
