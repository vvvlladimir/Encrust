# Updates

How the window learns about a release and replaces itself with it. Why it is built this way:
[ADR 0172](../decisions/0172-the-window-offers-a-signed-update-and-never-applies-one.md).
The code is `crates/encrust-app/src/updates/`.

## The feed

`release.yml`'s `feed` job uploads `latest.json` to every release:

```json
{
  "version": "0.3.0",
  "notes": "<the release body release-please wrote, markdown>",
  "platforms": {
    "macos-aarch64":  { "url": "<repository>/releases/download/v0.3.0/encrust-macos-aarch64.tar.gz",
                        "signature": "<the whole .minisig file>" },
    "macos-x86_64":   { "...": "..." },
    "linux-x86_64":   { "...": "..." },
    "windows-x86_64": { "url": ".../encrust-windows-x86_64.zip", "signature": "..." }
  }
}
```

The window reads `<repository>/releases/latest/download/latest.json`, `<repository>` being
`[workspace.package] repository`. GitHub answers that path only for the newest published
release, so a draft is never offered. A platform key is the archive name between `encrust-`
and the extension; a platform the feed has no key for still hears about the release and is
sent to the release page.

A version is `major.minor.patch` and compares as numbers. Nothing at or below the running
version is offered.

## What is checked before a byte is unpacked

1. The URL is `<repository>/releases/download/v<version>/<archive>`, with `<archive>` a bare
   file name. Anything else is refused before it is fetched.
2. The archive is fetched over TLS, at most 512 MiB.
3. Its minisign signature verifies against `assets/update-key.pub`, compiled in.
4. The signature's trusted comment, which the signature covers, reads exactly
   `encrust <version> <archive>`. A sound signature of `0.2.0` served as `0.3.0`, or of the
   Linux archive served to macOS, fails here.

## The swap

The archive holds `encrust/encrust` and `encrust/slice` (`.exe` on Windows). The new window
binary is written beside the running one as `encrust.new`, given the old file's permissions
and renamed over it, so a failure halfway leaves the old binary whole. `slice` is replaced
the same way, and only when it already stands beside the window.

Windows will not overwrite a running binary but will rename it, so there the running one
is first renamed to `encrust.old`; the next start deletes it. The path of the running
binary is read before the swap, because on Linux it reads `(deleted)` afterwards.

"Restart" closes the window through the usual unsaved-changes question and starts the new
binary as the process exits. "Keep working" in that question cancels the restart.

A development build (`debug_assertions`) is offered but never replaced, and so is a copy
started from an `.AppImage` (it runs from a read-only mount) or installed under `/usr` by the
`.deb` (the package manager owns it); those are sent to the release page. Inside `Encrust.app`
and the per-user Windows install the swap works as above.

## The signing key

The public half is `assets/update-key.pub`. The private half is the repository secret
`UPDATE_SIGNING_KEY`, the whole minisign secret key file, made without a password
(`minisign -G -W`) because the job has no terminal to type one into. It is kept outside
GitHub as well: without it no release can be signed for the builds already out.

To rotate it, make a new pair, replace `assets/update-key.pub` and the secret in one
release. Builds before it keep trusting the old key and must be updated by hand once.
