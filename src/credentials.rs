use std::{io, mem, ptr};
use windows_sys::Win32::Security::Credentials::{
    CredFree, CredReadW, CredUIPromptForCredentialsW, CredWriteW, CREDENTIALW,
    CREDUI_FLAGS_ALWAYS_SHOW_UI, CREDUI_FLAGS_DO_NOT_PERSIST, CREDUI_FLAGS_GENERIC_CREDENTIALS,
    CREDUI_INFOW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

const TARGET: &str = "SI OpenCode NAS (file.satreci.com)";

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

pub fn load() -> io::Result<Option<(String, String)>> {
    let mut credential = ptr::null_mut();
    let name = wide(TARGET);
    // SAFETY: CredReadW owns the returned allocation; CredFree is called below.
    if unsafe { CredReadW(name.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) } == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(1168) {
            return Ok(None);
        }
        return Err(error);
    }
    let result = unsafe {
        let item: &CREDENTIALW = &*credential;
        let username = wide_string(item.UserName);
        let bytes =
            std::slice::from_raw_parts(item.CredentialBlob, item.CredentialBlobSize as usize);
        let password = String::from_utf8(bytes.to_vec()).map_err(io::Error::other)?;
        Ok(Some((username, password)))
    };
    unsafe {
        CredFree(credential.cast());
    }
    result
}

pub fn prompt() -> io::Result<Option<(String, String)>> {
    let target = wide(TARGET);
    let caption = wide("SI OpenCode NAS sign in");
    let message = wide("Enter your FileStation account for update checks.");
    let info = CREDUI_INFOW {
        cbSize: mem::size_of::<CREDUI_INFOW>() as u32,
        hwndParent: ptr::null_mut(),
        pszMessageText: message.as_ptr(),
        pszCaptionText: caption.as_ptr(),
        hbmBanner: ptr::null_mut(),
    };
    let mut username = [0u16; 514];
    let mut password = [0u16; 256];
    let mut save = 0;
    let code = unsafe {
        CredUIPromptForCredentialsW(
            &info,
            target.as_ptr(),
            ptr::null(),
            0,
            username.as_mut_ptr(),
            username.len() as u32,
            password.as_mut_ptr(),
            password.len() as u32,
            &mut save,
            CREDUI_FLAGS_GENERIC_CREDENTIALS
                | CREDUI_FLAGS_ALWAYS_SHOW_UI
                | CREDUI_FLAGS_DO_NOT_PERSIST,
        )
    };
    if code == 1223 {
        return Ok(None);
    }
    if code != 0 {
        return Err(io::Error::from_raw_os_error(code as i32));
    }
    let result = (wide_slice(&username), wide_slice(&password));
    password.fill(0);
    Ok(Some(result))
}

pub fn save(username: &str, password: &str) -> io::Result<()> {
    let mut target = wide(TARGET);
    let mut user = wide(username);
    let mut secret = password.as_bytes().to_vec();
    let item = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        CredentialBlobSize: secret.len() as u32,
        CredentialBlob: secret.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: user.as_mut_ptr(),
        ..Default::default()
    };
    // SAFETY: all pointers remain valid until the call returns, and Windows copies the blob.
    let result = unsafe { CredWriteW(&item, 0) };
    secret.fill(0);
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

unsafe fn wide_string(mut pointer: *const u16) -> String {
    if pointer.is_null() {
        return String::new();
    }
    let mut len = 0;
    while *pointer != 0 {
        pointer = pointer.add(1);
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(pointer.sub(len), len))
}

fn wide_slice(chars: &[u16]) -> String {
    String::from_utf16_lossy(&chars[..chars.iter().position(|&c| c == 0).unwrap_or(chars.len())])
}
