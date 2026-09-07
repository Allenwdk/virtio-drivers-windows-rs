//! Opt-in startup diagnostics, readable over SSH after a failed PnP start.
//! The test installer creates Services\VirtioGpu\Parameters\Bringup first.

#[cfg(not(feature = "bringup-diagnostics"))]
pub fn record(_name: &str, _value: u32) {}

#[cfg(not(feature = "bringup-diagnostics"))]
pub fn event(_name: &str, _value: u32) {}

/// Preserve callback order without unbounded registry growth. These diagnostics
/// deliberately skip callbacks above PASSIVE_LEVEL, like `record` does.
#[cfg(feature = "bringup-diagnostics")]
pub fn event(name: &str, value: u32) {
    use core::sync::atomic::{AtomicU32, Ordering};
    static SEQUENCE: AtomicU32 = AtomicU32::new(0);
    if unsafe { wdk::wdm::KeGetCurrentIrql() } != 0 { return; }
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    if sequence < 256 {
        record(&alloc::format!("Event{sequence:03}_{name}"), value);
    }
}

#[cfg(feature = "bringup-diagnostics")]
pub fn record(name: &str, value: u32) {
    use core::{mem::zeroed, ptr::null_mut};
    use wdk::wdm::*;
    // Registry APIs require PASSIVE_LEVEL. Never change callback behavior just
    // to emit a diagnostic, and never log an error recursively from this path.
    if unsafe { KeGetCurrentIrql() } != 0 { return; }
    let mut path: alloc::vec::Vec<u16> = "\\Registry\\Machine\\System\\CurrentControlSet\\Services\\VirtioGpu\\Parameters\\Bringup".encode_utf16().collect();
    let mut key_name = UNICODE_STRING {
        Length: (path.len() * 2) as u16,
        MaximumLength: (path.len() * 2) as u16,
        Buffer: path.as_mut_ptr(),
    };
    let mut attrs: OBJECT_ATTRIBUTES = unsafe { zeroed() };
    attrs.Length = size_of::<OBJECT_ATTRIBUTES>() as u32;
    attrs.ObjectName = &mut key_name;
    attrs.Attributes = OBJ_CASE_INSENSITIVE | OBJ_KERNEL_HANDLE;
    let mut handle = null_mut();
    let status = unsafe { ZwOpenKey(&mut handle, KEY_SET_VALUE, &mut attrs) };
    if (status as i32) < 0 { return; }
    let mut name_buf: alloc::vec::Vec<u16> = name.encode_utf16().collect();
    let mut value_name = UNICODE_STRING {
        Length: (name_buf.len() * 2) as u16,
        MaximumLength: (name_buf.len() * 2) as u16,
        Buffer: name_buf.as_mut_ptr(),
    };
    unsafe {
        ZwSetValueKey(handle, &mut value_name, 0, REG_DWORD,
                      (&value as *const u32).cast_mut().cast(), size_of::<u32>() as u32);
        ZwClose(handle);
    }
}
