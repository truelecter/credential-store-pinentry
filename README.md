# credential-store-pinentry

A [pinentry](https://www.gnupg.org/related_software/pinentry/) for `gpg-agent` that answers the OpenPGP card (YubiKey) user-PIN prompt from the operating system's credential store, without a dialog:

- macOS: the login Keychain
- Windows: Credential Manager

Every other prompt goes to a real pinentry: the Admin PIN, a new PIN, key passphrases, a card with no stored PIN, and any retry after a wrong PIN. It never sends a stored PIN twice, so a wrong stored PIN costs one card attempt at most.

## Store the PIN

The PIN is looked up per card serial number, as `gpg --card-status` prints it without spaces (`Serial number ....: 32898543`).

macOS (trust `/usr/bin/security`, which does the lookup, so it never asks for Keychain access):

```sh
security add-generic-password -s gpg-card-pin -a 32898543 -T /usr/bin/security -U -w
```

Windows (a generic credential named `gpg-card-pin/<serial>`):

```powershell
cmdkey /generic:gpg-card-pin/32898543 /user:32898543 /pass
```

## Configure gpg-agent

In `gpg-agent.conf` (`~/.gnupg/` on macOS, `gpgconf --list-dirs homedir` on Windows):

```
pinentry-program /path/to/credential-store-pinentry
```

Then `gpgconf --kill gpg-agent`.

Environment variables, read from `gpg-agent`'s environment:

| Variable | Default |
| --- | --- |
| `CREDENTIAL_STORE_PINENTRY_SERVICE` | `gpg-card-pin` |
| `CREDENTIAL_STORE_PINENTRY_FALLBACK` | `pinentry-mac` on macOS, `pinentry-basic.exe` on Windows, looked up on `PATH` |

On Windows, set them as user environment variables (`setx CREDENTIAL_STORE_PINENTRY_FALLBACK C:\path\to\pinentry.exe`) before `gpg-agent` starts. Any pinentry works as the fallback, for example Gpg4win's `pinentry.exe` or [pinentry-for-windows](https://github.com/baliestri/pinentry-for-windows).

## Install

- Windows: download `credential-store-pinentry.exe` from the [releases](https://github.com/truelecter/credential-store-pinentry/releases), or build it with `cargo build --release`.
- Nix: `packages.aarch64-darwin.default` (macOS) and `packages.x86_64-linux.windows` (the Windows `.exe`, cross-built). `overlays.default` adds `credential-store-pinentry` on macOS and `credential-store-pinentry-windows` on x86_64.

## License

MIT
