//! credential-store-pinentry: answer the OpenPGP card user-PIN prompt from the OS credential store
//! (macOS Keychain, Windows Credential Manager) without a dialog. Every other prompt (Admin PIN, new PIN,
//! key passphrases, a retry after a bad PIN, a card with no stored PIN) goes to a real pinentry.
//!
//!   CREDENTIAL_STORE_PINENTRY_SERVICE   credential name, default gpg-card-pin
//!   CREDENTIAL_STORE_PINENTRY_FALLBACK  real pinentry, default pinentry-mac / pinentry-basic.exe on PATH

mod prompt;
mod store;

use prompt::{Prompt, encode};
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

const DEFAULT_SERVICE: &str = "gpg-card-pin";

#[cfg(target_os = "macos")]
const DEFAULT_FALLBACK: &str = "pinentry-mac";
#[cfg(windows)]
const DEFAULT_FALLBACK: &str = "pinentry-basic.exe";
#[cfg(not(any(target_os = "macos", windows)))]
const DEFAULT_FALLBACK: &str = "pinentry";

fn main() {
    let service = std::env::var("CREDENTIAL_STORE_PINENTRY_SERVICE").unwrap_or_else(|_| DEFAULT_SERVICE.into());
    let fallback = std::env::var("CREDENTIAL_STORE_PINENTRY_FALLBACK").unwrap_or_else(|_| DEFAULT_FALLBACK.into());
    let code = match run(&service, &fallback) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("credential-store-pinentry: {err}");
            1
        }
    };
    std::process::exit(code);
}

fn send(out: &mut impl Write, line: &str) -> io::Result<()> {
    out.write_all(line.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}

fn run(service: &str, fallback: &str) -> io::Result<i32> {
    let mut input = BufReader::new(io::stdin());
    let mut out = io::stdout().lock();
    // Commands with a one-line reply answered so far, replayed to the fallback.
    let mut seen: Vec<Vec<u8>> = Vec::new();
    let mut state = Prompt::default();

    send(&mut out, "OK credential-store-pinentry ready")?;
    let mut raw = Vec::new();
    loop {
        raw.clear();
        if input.read_until(b'\n', &mut raw)? == 0 {
            return Ok(0);
        }
        while raw.last().is_some_and(|b| *b == b'\n' || *b == b'\r') {
            raw.pop();
        }
        let line = String::from_utf8_lossy(&raw).into_owned();
        let (cmd, arg) = line.split_once(' ').unwrap_or((&line, ""));
        match cmd {
            "SETDESC" => state.desc = arg.into(),
            "SETPROMPT" => state.prompt = arg.into(),
            "SETERROR" => state.had_error = true,
            "GETINFO" => {
                // gpg-agent cancels a pinentry by killing the pid reported here: this process.
                if arg == "pid" {
                    send(&mut out, &format!("D {}", std::process::id()))?;
                }
                send(&mut out, "OK")?;
                continue;
            }
            "GETPIN" => {
                if let Some(pin) = state.card_serial().and_then(|serial| store::lookup(service, &serial)) {
                    state.answered = true;
                    send(&mut out, &format!("D {}", encode(&pin)))?;
                    send(&mut out, "OK")?;
                    continue;
                }
                return hand_off(fallback, &seen, &raw, input, out);
            }
            "CONFIRM" | "MESSAGE" => return hand_off(fallback, &seen, &raw, input, out),
            "BYE" => {
                send(&mut out, "OK")?;
                return Ok(0);
            }
            _ => {}
        }
        send(&mut out, "OK")?;
        seen.push(raw.clone());
    }
}

/// Replay the session to the real pinentry, drop the replies gpg-agent already got (greeting plus one OK
/// per replayed command), then proxy the rest until the fallback exits, and exit with its status.
fn hand_off(
    fallback: &str,
    seen: &[Vec<u8>],
    current: &[u8],
    mut input: BufReader<io::Stdin>,
    mut out: io::StdoutLock,
) -> io::Result<i32> {
    let mut child = Command::new(fallback)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| io::Error::new(err.kind(), format!("starting {fallback}: {err}")))?;
    die_with_parent(&child);

    let mut child_in = child.stdin.take().expect("piped stdin");
    let mut replay = Vec::new();
    for line in seen.iter().map(Vec::as_slice).chain([current]) {
        replay.extend_from_slice(line);
        replay.push(b'\n');
    }
    // A thread, so a fallback that answers before reading everything never deadlocks us. It also forwards
    // whatever `input` already buffered past the current line.
    std::thread::spawn(move || {
        if child_in.write_all(&replay).is_ok() {
            let _ = io::copy(&mut input, &mut child_in);
        }
    });

    let mut skip = seen.len() + 1;
    let mut reader = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        if skip > 0 {
            skip -= 1;
            continue;
        }
        out.write_all(&line)?;
        out.flush()?;
    }
    Ok(child.wait()?.code().unwrap_or(1))
}

/// gpg-agent cancels a prompt by killing this process; take the fallback (and its dialog) down with it.
#[cfg(unix)]
fn die_with_parent(child: &Child) {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    let pid = child.id() as libc::pid_t;
    let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGINT, SIGTERM, SIGHUP]) else {
        return;
    };
    std::thread::spawn(move || {
        if signals.forever().next().is_some() {
            // SAFETY: plain kill(2) on the fallback's pid.
            unsafe { libc::kill(pid, libc::SIGTERM) };
            std::process::exit(1);
        }
    });
}

/// On Windows gpg-agent uses TerminateProcess, which no handler sees. A kill-on-close job object ends the
/// fallback when the last handle to the job, held by this process until it dies, goes away.
#[cfg(windows)]
fn die_with_parent(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    // SAFETY: Win32 calls on a fresh job handle and the child's live process handle. The job handle is
    // deliberately never closed: the OS closes it when this process exits, which is the point.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw const info).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        AssignProcessToJobObject(job, child.as_raw_handle().cast());
    }
}

#[cfg(not(any(unix, windows)))]
fn die_with_parent(_child: &Child) {}
