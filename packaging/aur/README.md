# AUR

Arch users install with `yay -S nopass` (or `paru`, or a plain
`makepkg -si`) once this is pushed.

## TODO: nopass is not on the AUR yet

Nothing here has run. The account does not exist, the name is unclaimed, and
`nopass` is not installable on Arch. Blocked on step 1 below:
<https://aur.archlinux.org/register> was erroring as of 2026-08-08 — the site
503s under load, so it is worth retrying rather than giving up.

Once the account and key exist, do the [first push](#publishing) by hand —
that is what creates the package — then add the private key as the
`AUR_SSH_KEY` repository secret. From then on the `aur` job in
[`publish.yml`](../../.github/workflows/publish.yml) updates it on every tag,
and the rest of this file is background reading.

## One-time setup

1. Make an account at <https://aur.archlinux.org/register>.
2. Add your **public** SSH key under *My Account → SSH Public Key*.
3. Point ssh at it, if it is not your default key:

   ```
   # ~/.ssh/config
   Host aur.archlinux.org
     User aur
     IdentityFile ~/.ssh/aur
   ```

Check it with `ssh aur@aur.archlinux.org help` — it should print a command
list rather than `Permission denied (publickey)`.

## Publishing

The AUR is a git remote; pushing to a name nobody has taken creates the
package.

```sh
git clone ssh://aur@aur.archlinux.org/nopass.git aur-nopass
cd aur-nopass
cp ../packaging/aur/PKGBUILD ../packaging/aur/.SRCINFO .
git add PKGBUILD .SRCINFO
git commit -m "nopass 0.2.0"
git push
```

Only `PKGBUILD` and `.SRCINFO` belong in that repository — no README, no
source tarball.

## Every release afterwards

1. Bump `pkgver` and reset `pkgrel=1` in `PKGBUILD`.
2. Replace `sha256sums` with the new tarball's checksum:

   ```sh
   curl -fsSL https://github.com/souravsspace/nopass/archive/refs/tags/v0.3.0.tar.gz | sha256sum
   ```

3. Regenerate the metadata — the checked-in `.SRCINFO` was written by hand,
   so on an Arch machine let makepkg do it instead:

   ```sh
   makepkg --printsrcinfo > .SRCINFO
   ```

4. Build it once before pushing: `makepkg -si` (this also runs the test
   suite, which the PKGBUILD's `check()` invokes).
