# AGENTS.md

A pinentry for `gpg-agent` that answers the OpenPGP card user PIN from the macOS Keychain or Windows Credential Manager and proxies every other prompt to a fallback pinentry.

## Layout

```text
src/main.rs        Assuan loop, hand-off to the fallback, killing the fallback with this process
src/prompt.rs      which prompts may be answered (`Prompt::card_serial`), Assuan escaping
src/store.rs       credential lookup per platform (Keychain through /usr/bin/security, CredReadW)
tests/hand_off.rs  replay to a fake fallback (unix)
package.nix        buildRustPackage, used native and with pkgsCross.mingwW64
flake.nix          packages, checks, overlays.default
.github/           build on push, Windows release on tags
```

## Rules

- Never answer a prompt that may be a retry: `Prompt::card_serial` must keep refusing after `SETERROR`, after one answer, and on "Remaining attempts", Admin, New and Reset prompts. A wrong stored PIN must cost one card attempt at most.
- The hand-off replays every command answered with one line, then drops exactly that many replies plus the greeting. A command answered locally (`GETINFO`, a stored PIN) is not replayed.
- macOS reads the Keychain through `/usr/bin/security`, so the item's access list (`-T /usr/bin/security`) covers every build of this binary.

## Test a change

- `nix flake check`, or `nix build .#checks.<system>.tests`: unit tests and the hand-off test.
- `nix build .#packages.x86_64-linux.windows`: the Windows cross build.
- In a consumer, try a checkout before pushing: `--override-input credential-store-pinentry path:<this checkout>`.

## Commits

- Conventional Commits, scoped by area: `pinentry`, `nix`, `ci`, `repo`. For example `fix(pinentry): ...`.
