//! C ABI used by the Qt/QML desktop shell.

use std::{
    ffi::{CStr, CString, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
};

use kubeweft_filesystem::{EntryMetadata, FileType};
use kubeweft_local::LocalFilesystem;
use serde_json::{Value, json};

pub struct DesktopFilesystem(LocalFilesystem);

#[unsafe(no_mangle)]
/// # Safety
/// `data_directory` must point to a NUL-terminated string for this call.
pub unsafe extern "C" fn kubeweft_desktop_open(
    data_directory: *const c_char,
) -> *mut DesktopFilesystem {
    catch_unwind(AssertUnwindSafe(|| {
        let data_directory = c_string(data_directory)?;
        let filesystem = LocalFilesystem::open(data_directory).ok()?;
        filesystem.bootstrap_home("user").ok()?;
        Some(Box::into_raw(Box::new(DesktopFilesystem(filesystem))))
    }))
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `filesystem` must be a live handle returned by `kubeweft_desktop_open` and
/// must not be used or closed again after this call.
pub unsafe extern "C" fn kubeweft_desktop_close(filesystem: *mut DesktopFilesystem) {
    if !filesystem.is_null() {
        // SAFETY: pointers returned by `kubeweft_desktop_open` are uniquely owned
        // by the caller until this function is invoked once.
        unsafe { drop(Box::from_raw(filesystem)) };
    }
}

#[unsafe(no_mangle)]
/// # Safety
/// The handle must be live and `path` must be a NUL-terminated string.
pub unsafe extern "C" fn kubeweft_desktop_list(
    filesystem: *mut DesktopFilesystem,
    path: *const c_char,
) -> *mut c_char {
    call(|| {
        let path = c_string(path).ok_or("invalid UTF-8 path")?;
        let entries = local(filesystem)?.0.service().list(path).map_err(message)?;
        let values: Vec<Value> = entries
            .into_iter()
            .map(|entry| {
                let kind = match entry.metadata.file_type() {
                    FileType::File => "file",
                    FileType::Directory => "directory",
                };
                let size = match &entry.metadata {
                    EntryMetadata::File(file) => file.size,
                    EntryMetadata::Directory(_) => 0,
                };
                json!({
                    "name": entry.name,
                    "kind": kind,
                    "size": size,
                    "generation": entry.metadata.generation(),
                })
            })
            .collect();
        Ok(json!({ "entries": values }))
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// The handle must be live and `path` must be a NUL-terminated string.
pub unsafe extern "C" fn kubeweft_desktop_read(
    filesystem: *mut DesktopFilesystem,
    path: *const c_char,
) -> *mut c_char {
    call(|| {
        let path = c_string(path).ok_or("invalid UTF-8 path")?;
        let data = local(filesystem)?.0.service().read(path).map_err(message)?;
        let content = String::from_utf8(data).map_err(|_| "file is not UTF-8 text".to_owned())?;
        let metadata = local(filesystem)?.0.service().stat(path).map_err(message)?;
        Ok(json!({
            "content": content,
            "generation": metadata.generation(),
        }))
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// The handle must be live and `path` must be a NUL-terminated string.
pub unsafe extern "C" fn kubeweft_desktop_mkdir(
    filesystem: *mut DesktopFilesystem,
    path: *const c_char,
) -> *mut c_char {
    call(|| {
        let path = c_string(path).ok_or("invalid UTF-8 path")?;
        local(filesystem)?
            .0
            .service()
            .mkdir(path)
            .map_err(message)?;
        Ok(json!({}))
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// The handle must be live and `path` must be a NUL-terminated string.
pub unsafe extern "C" fn kubeweft_desktop_create(
    filesystem: *mut DesktopFilesystem,
    path: *const c_char,
) -> *mut c_char {
    call(|| {
        let path = c_string(path).ok_or("invalid UTF-8 path")?;
        let metadata = local(filesystem)?
            .0
            .service()
            .create(path)
            .map_err(message)?;
        Ok(json!({ "generation": metadata.generation }))
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// The handle must be live; `path` and `content` must be NUL-terminated strings.
pub unsafe extern "C" fn kubeweft_desktop_write(
    filesystem: *mut DesktopFilesystem,
    path: *const c_char,
    content: *const c_char,
    expected_generation: u64,
) -> *mut c_char {
    call(|| {
        let path = c_string(path).ok_or("invalid UTF-8 path")?;
        let content = c_string(content).ok_or("invalid UTF-8 content")?;
        let local = local(filesystem)?;
        let metadata = local
            .0
            .service()
            .write(
                path,
                content.as_bytes(),
                Some(expected_generation),
                local.0.device_id(),
            )
            .map_err(message)?;
        Ok(json!({ "generation": metadata.generation }))
    })
}

#[unsafe(no_mangle)]
/// # Safety
/// The handle must be live and `path` must be a NUL-terminated string.
pub unsafe extern "C" fn kubeweft_desktop_remove(
    filesystem: *mut DesktopFilesystem,
    path: *const c_char,
) -> *mut c_char {
    call(|| {
        let path = c_string(path).ok_or("invalid UTF-8 path")?;
        local(filesystem)?
            .0
            .service()
            .remove(path)
            .map_err(message)?;
        Ok(json!({}))
    })
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `value` must be a result returned by one of the bridge operations and must
/// not be read or freed again after this call.
pub unsafe extern "C" fn kubeweft_desktop_string_free(value: *mut c_char) {
    if !value.is_null() {
        // SAFETY: result strings are allocated by `call` and transferred to the
        // caller, which returns each allocation here exactly once.
        unsafe { drop(CString::from_raw(value)) };
    }
}

fn call(operation: impl FnOnce() -> Result<Value, String>) -> *mut c_char {
    let response = catch_unwind(AssertUnwindSafe(operation))
        .map_err(|_| "desktop bridge panicked".to_owned())
        .and_then(|result| result)
        .map(|value| json!({ "ok": true, "value": value }))
        .unwrap_or_else(|error| json!({ "ok": false, "error": error }));
    let encoded = response.to_string().replace('\0', "\\u0000");
    CString::new(encoded)
        .expect("JSON response cannot contain an interior NUL")
        .into_raw()
}

fn local<'a>(filesystem: *mut DesktopFilesystem) -> Result<&'a mut DesktopFilesystem, String> {
    if filesystem.is_null() {
        return Err("filesystem is not open".into());
    }
    // SAFETY: the Qt owner serializes calls on its GUI thread and keeps the
    // handle alive for the full operation.
    Ok(unsafe { &mut *filesystem })
}

fn c_string<'a>(value: *const c_char) -> Option<&'a str> {
    if value.is_null() {
        return None;
    }
    // SAFETY: bridge callers pass non-null, NUL-terminated strings that remain
    // valid for the duration of the call.
    unsafe { CStr::from_ptr(value) }.to_str().ok()
}

fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::CString,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::Value;

    use super::*;

    fn temporary_directory() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "kubeweft-desktop-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn decode(value: *mut c_char) -> Value {
        let result = unsafe { CStr::from_ptr(value) }
            .to_str()
            .unwrap()
            .to_owned();
        unsafe { kubeweft_desktop_string_free(value) };
        serde_json::from_str(&result).unwrap()
    }

    #[test]
    fn bridge_exposes_the_local_namespace() {
        let directory = temporary_directory();
        let directory_string = CString::new(directory.to_str().unwrap()).unwrap();
        let handle = unsafe { kubeweft_desktop_open(directory_string.as_ptr()) };
        assert!(!handle.is_null());

        let home = CString::new("/home/user").unwrap();
        let file = CString::new("/home/user/hello.txt").unwrap();
        assert_eq!(
            decode(unsafe { kubeweft_desktop_create(handle, file.as_ptr()) })["ok"],
            true
        );
        let content = CString::new("hello desktop").unwrap();
        assert_eq!(
            decode(unsafe { kubeweft_desktop_write(handle, file.as_ptr(), content.as_ptr(), 1) })["ok"],
            true
        );
        let listing = decode(unsafe { kubeweft_desktop_list(handle, home.as_ptr()) });
        assert_eq!(listing["value"]["entries"][0]["name"], "hello.txt");
        assert_eq!(
            decode(unsafe { kubeweft_desktop_read(handle, file.as_ptr()) })["value"]["content"],
            "hello desktop"
        );

        unsafe { kubeweft_desktop_close(handle) };
        fs::remove_dir_all(directory).unwrap();
    }
}
