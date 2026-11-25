//! The OS credential store: macOS Keychain or Windows Credential Manager.

/// The stored PIN for card `serial`, if any.
#[cfg(target_os = "macos")]
pub fn lookup(service: &str, serial: &str) -> Option<String> {
    // Through /usr/bin/security, not the Security framework: the item's access list trusts that binary
    // (`-T /usr/bin/security`), so a rebuilt pinentry never triggers a Keychain permission prompt.
    let out = std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", service, "-a", serial, "-w"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let pin = String::from_utf8(out.stdout).ok()?;
    let pin = pin.strip_suffix('\n').unwrap_or(&pin);
    (!pin.is_empty()).then(|| pin.to_owned())
}

/// The stored PIN for card `serial`, from the generic credential `<service>/<serial>`.
#[cfg(windows)]
pub fn lookup(service: &str, serial: &str) -> Option<String> {
    use windows_sys::Win32::Security::Credentials::{CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW};

    let target: Vec<u16> = format!("{service}/{serial}").encode_utf16().chain([0]).collect();
    let mut cred: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: `target` is NUL-terminated; on success `cred` points to a CREDENTIALW that we free below.
    if unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut cred) } == 0 {
        return None;
    }
    // SAFETY: CredReadW succeeded, so `cred` and its blob of `CredentialBlobSize` bytes are valid until CredFree.
    let blob = unsafe {
        let c = &*cred;
        std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec()
    };
    // SAFETY: `cred` came from CredReadW and is freed once.
    unsafe { CredFree(cred.cast()) };

    let pin = decode_blob(blob)?;
    (!pin.is_empty()).then_some(pin)
}

/// cmdkey and the Credential Manager UI store UTF-16LE, other tools often raw UTF-8. A wrong guess sends a
/// wrong PIN and burns a card attempt, so UTF-16 only when it is unambiguous: every high byte zero.
#[cfg(any(windows, test))]
fn decode_blob(blob: Vec<u8>) -> Option<String> {
    let utf16 = blob.len() % 2 == 0 && blob.iter().skip(1).step_by(2).all(|&b| b == 0);
    if utf16 {
        let wide: Vec<u16> = blob.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
        String::from_utf16(&wide).ok()
    } else {
        String::from_utf8(blob).ok()
    }
}

/// No credential store on this platform: every prompt goes to the fallback pinentry.
#[cfg(not(any(target_os = "macos", windows)))]
pub fn lookup(_service: &str, _serial: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::decode_blob;

    #[test]
    fn cmdkey_utf16_blob() {
        let blob = "123456".encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decode_blob(blob).as_deref(), Some("123456"));
    }

    #[test]
    fn even_length_utf8_blob_is_not_read_as_utf16() {
        assert_eq!(decode_blob(b"123456".to_vec()).as_deref(), Some("123456"));
    }
}
