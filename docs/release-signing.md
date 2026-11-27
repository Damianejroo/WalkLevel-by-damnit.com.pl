# Release signing

English · [Polski](release-signing.pl.md)

Since 0.6.4 the **Update** button installs only an installer signed with the owner's key. The
public key is built into the app (`SIGNING_PUBLIC_KEY` in `crates/core/src/updates.rs`), the
private key lives only with the owner and in the repository's Actions secrets. The signature
is a [minisign](https://jedisct1.github.io/minisign/) file next to the installer in the release:
`WalkLevel-Setup-X.Y.Z.exe.sig`.

Keys and signatures are made with `tauri signer`, which comes with the project
(`@tauri-apps/cli` in `package.json`), so nothing else needs to be installed. The app checks
them with the [`minisign-verify`](https://crates.io/crates/minisign-verify) crate (no
dependencies; the same crate the Tauri updater uses).

## One-time setup (owner)

Run these in PowerShell in the repository folder, after `npm ci`.

1. **Generate the key pair.** Keep it outside the repository:

   ```powershell
   New-Item -ItemType Directory -Force "$env:USERPROFILE\.walklevel" | Out-Null
   npx tauri signer generate -w "$env:USERPROFILE\.walklevel\walklevel-signing.key"
   ```

   It asks for a password twice. Choose a long one and save it in your password manager first.
   The result is `walklevel-signing.key` (private key, encrypted with the password) and
   `walklevel-signing.key.pub` (public key).

2. **Put the public key into the app.** Copy the single line of the `.pub` file:

   ```powershell
   Get-Content "$env:USERPROFILE\.walklevel\walklevel-signing.key.pub" | Set-Clipboard
   ```

   In `crates/core/src/updates.rs` replace the text of `SIGNING_PUBLIC_KEY`
   (`"PLACEHOLDER: …"`) with that line, in quotes. Then check and commit:

   ```powershell
   cargo test -p walklevel-core --lib updates
   ```

   The test `built_in_key_is_placeholder_or_valid` fails if the line is not a minisign public
   key. While the placeholder is there, a release build (`cargo build --release`,
   `packaging/build.ps1`) stops with a compile error, and so does the release workflow.

3. **Add the private key and its password as Actions secrets** (needs `gh auth login`):

   ```powershell
   Get-Content -Raw "$env:USERPROFILE\.walklevel\walklevel-signing.key" | gh secret set WALKLEVEL_SIGNING_KEY --repo damnit-damian/WalkLevel-by-damnit.com.pl
   gh secret set WALKLEVEL_SIGNING_KEY_PASSWORD --repo damnit-damian/WalkLevel-by-damnit.com.pl
   ```

   The second command asks for the value: paste the password. Without `gh`: on GitHub open
   the repository, **Settings > Secrets and variables > Actions > New repository secret**, and
   add both secrets with these names (the value of `WALKLEVEL_SIGNING_KEY` is the whole
   content of the `.key` file).

4. **Keep a safe copy of the private key.** Store the `.key` file (as an attachment or its text
   as a secure note) and the password in your password manager, and optionally on an encrypted
   USB stick kept offline. Never put the key in the repository, in a synced folder without
   encryption, in an email or in a chat. Whoever has the key and the password can publish an
   update that every installed WalkLevel accepts.

## What the release workflow does

`.github/workflows/release.yml`, job `budowa`:

1. Stops at once if `WALKLEVEL_SIGNING_KEY` or `WALKLEVEL_SIGNING_KEY_PASSWORD` is missing, or if
   `SIGNING_PUBLIC_KEY` is still the placeholder.
2. Builds the exe and `WalkLevel-Setup-X.Y.Z.exe` (`packaging/build.ps1`).
3. Signs it: `npx tauri signer sign --app-version X.Y.Z` writes `WalkLevel-Setup-X.Y.Z.exe.sig`.
   The version goes into the signature's trusted comment (`version:X.Y.Z`).
4. Checks the new signature with the key built into the app (an ignored test in
   `updates.rs`, run only here). If the secret and the public key are not one pair, the release
   stops instead of shipping an update nobody can install.
5. 0.6.4 only (`LEGACY_INSTALLER_COPY`): copies the installer, `.sha256` and `.sig` under the
   old name `Bieznia-Setup-X.Y.Z.exe` for the Update button of 0.6.3.
6. Adds the provenance attestation and creates the release draft with the installer, `.sha256`
   and `.sig`; publishing happens after the tests pass.

## How the app checks an update

1. The installer must come from
   `https://github.com/damnit-damian/WalkLevel-by-damnit.com.pl/releases/download/<tag>/<file>`
   (redirects only to GitHub's download servers).
2. It downloads `<installer>.sig` first. No `.sig` in the release: nothing is downloaded and the
   app shows `aktualizacja.powod.signature-missing`.
3. It downloads the installer and checks the SHA-256 from GitHub.
4. It checks the minisign signature (Ed25519 over the BLAKE2b hash of the file) with
   `SIGNING_PUBLIC_KEY`, and that the trusted comment names the release's version. A damaged
   signature, another key, another file or another version: the file is deleted and the app
   shows `aktualizacja.powod.signature-invalid`.
5. Only then does it start the installer.

## Signing by hand

For a local test of a self-built installer:

```powershell
npx tauri signer sign -f "$env:USERPROFILE\.walklevel\walklevel-signing.key" --app-version X.Y.Z dist-installer\WalkLevel-Setup-X.Y.Z.exe
```

## A new key

If the key is lost, installed apps can no longer be updated with the button: users have to
download the next installer by hand. If the key leaks, make a new pair at once, put the new
public key into the app, sign that release with the **old** key one last time (installed apps
trust only the old one), then replace both secrets. Every later release is signed with the new
key.

## Tests

`crates/core/tests/fixtures/updates/signatures.json` holds TEST ONLY keys and signatures made
with `tauri signer` for the unit tests (good signature, damaged signature, missing signature,
signature of another file, of another version and by another key). Their private keys were not
kept, and the app never trusts them.
