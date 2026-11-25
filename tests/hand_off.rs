#![cfg(unix)]
//! Hand-off to the fallback pinentry: the replayed commands' replies must not reach gpg-agent twice.

use std::io::Write;
use std::process::{Command, Stdio};

const FAKE_PINENTRY: &str = r#"
echo "OK fake ready"
while IFS= read -r line; do
  case $line in
    GETPIN) echo "D from-fallback"; echo OK ;;
    BYE) echo OK; exit 0 ;;
    *) echo OK ;;
  esac
done
"#;

#[test]
fn passphrase_prompt_is_proxied_once() {
    let dir = std::env::temp_dir().join(format!("csp-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fake = dir.join("fake-pinentry");
    std::fs::write(&fake, format!("#!/bin/sh\n{FAKE_PINENTRY}")).unwrap();
    std::fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_credential-store-pinentry"))
        .env("CREDENTIAL_STORE_PINENTRY_FALLBACK", &fake)
        .env("CREDENTIAL_STORE_PINENTRY_SERVICE", "credential-store-pinentry-test-none")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"SETDESC Please enter the passphrase%0Afor key X\nSETPROMPT Passphrase:\nGETINFO pid\nGETPIN\nBYE\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    let stdout = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 8, "{stdout}");
    assert_eq!(&lines[..3], ["OK credential-store-pinentry ready", "OK", "OK"]);
    assert!(lines[3].starts_with("D "), "{stdout}");
    assert_eq!(&lines[4..], ["OK", "D from-fallback", "OK", "OK"]);
    assert!(out.status.success());
}
