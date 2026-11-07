# Signed extension updates

`root.json` is the public trust anchor embedded in Manatan. It contains no
private keys. Updating a community catalog entry does not grant authority to
replace an installed extension automatically.

Build first-party packages with `scripts/package-extensions.sh`. Install the
publisher requirements in an operator virtual environment:

```sh
python3 -m venv /path/to/publisher-venv
/path/to/publisher-venv/bin/pip install -r scripts/extension-repository-requirements.txt
/path/to/publisher-venv/bin/python scripts/extension-repository.py publish \
  --keys /protected/extension-signing --out /public/extension-repository \
  --packages target/extension-packages --rollout 10
```

The existing repository and signing keys must persist between runs. Never
reinitialize them for each deployment: metadata versions prevent replay.
`init` is only for a new publisher identity. The first-party identity already
has its public root checked in here; its private keys must stay outside source
and public output. Keep at least two of three root keys offline. Routine
publishing needs only targets, snapshot, and timestamp keys.

Run `publish` at least daily, even without packages, to refresh the seven-day
snapshot/timestamp expiry. Increase rollout after validation. Packages are
immutable: changed bytes require a new version. Retain old targets and
versioned metadata so existing compatible clients can finish their update.

To withdraw a faulty package, use `publish --withdraw id-version.mext`.
Clients with automatic updates enabled restore their retained previous
version; paused clients show the withdrawal and await user action. Local
runtime failures also trigger rollback and quarantine without waiting for a
publisher withdrawal. User rollback pauses automatic updates.

Use `rotate-root` with the offline root keys to rotate the root trust keys.
Both the old and new thresholds must sign; retain every numbered root file.
Move new root keys back offline after rotation. Online role-key rotation is a
separate root-metadata change and is not performed by this helper.

Deploy only the public `metadata/` and `targets/` directories. Upload immutable
files first and `timestamp.json` last. Configure the website's read-only
`MANATAN_EXTENSION_UPDATE_REPOSITORY` mount to serve this directory at
`/api/srs/extension-updates/`. Do not expose keys, and do not delete historical
files during a rolling deploy. Publishing this local directory does not deploy
it. The app will keep working with its bundled/installed code if a feed is
unavailable or expired.

The host currently supports API/state version 1. State changes must stay
backward compatible with the retained previous package. A package requiring a
new host API or irreversible state format needs an app compatibility change;
it must not silently migrate state and then claim rollback is safe.

`systemd/manatan-extension-metadata.service` and its daily persistent timer
provide the unattended metadata refresh job for a Linux publisher host. Adjust
paths for the operator environment, install the pinned virtual environment and
SDK checkout, create the unprivileged `manatan-publisher` user, grant it write
access only to the persistent public repository, and read access to the three
online keys. Keep all root private keys offline. Install/enable the timer via
the normal deployment process; these templates do not enable a live job.
Point the website's read-only mount at the same public directory. New package
publication must run as this same publisher. The CLI takes a repository file
lock so a release publication and scheduled refresh cannot race their metadata
counters or temporary files. Keep the repository on a local filesystem that
supports POSIX file locks.
