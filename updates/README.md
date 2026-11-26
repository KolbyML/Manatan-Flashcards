# Official Flashcards extension updates

The official feed uses one Ed25519 publisher key. A single signature over
`metadata/releases.json` authenticates the release list and each package's
SHA-256 and size. The app pins the key in `updates/publisher.pub` and caches
installed packages on disk. The catalog sequence advances when a release is
published or withdrawn. The app rejects older catalogs after seeing a newer
sequence. The catalog has no expiration timer.

## GitHub Actions

Every push to `master` tests the workspace and builds the three `.mext` files.
To publish, manually run **Flashcards extensions** from `master`, select one
package or all three, set the rollout, and approve the production job. The
`extension-production` environment uses one secret,
`EXTENSION_SIGNING_PUBLISHER_PEM`: the existing Ed25519 publisher private
key whose public half is in `updates/publisher.pub`. The job pushes the
signed catalog and package bytes to `extension-feed` with the workflow token.

Package versions are immutable. Bump the manifest version if package bytes
change. To withdraw a version, use the same signer with
`--withdraw extension-version.mext` against the checked-out feed before
publishing it.

## Serve the official feed

Create the `extension-feed` branch with a public README on initial setup. The
release workflow writes public metadata and targets there. Get Shared and the
native updater read those files directly from GitHub's HTTPS static host.
Preserve the branch's catalog and targets across runs so sequence numbers and
packages survive. Verify the public catalog and an in-app update before shipping.

An existing website mirror can still serve the same signed files using
`scripts/sync-extension-feed.py`. Keep any existing legacy TUF endpoint available
for released clients using it.

## Third-party feeds

A publisher can host their own packages and catalog on any HTTPS static host.
Create a separate Ed25519 private PEM and a public `.pub` file containing the
raw 32-byte public key as 64 hexadecimal characters. Keep the private key
offline or in the publisher's CI secrets. Build the extension's `.mext`
using the SDK and sign the releases with that key.

```sh
python3 -m pip install -r scripts/extension-signing-requirements.txt
python3 scripts/publish-extension-feed.py \
  --key /private/publisher.pem --publisher-public-key publisher.pub \
  --publisher-id my-publisher --publisher-name 'My Publisher' \
  --packages dist/ --out /public/flashcards-feed
```

Serve `metadata/` and `targets/` from the feed's base URL and give users the
public key through a channel they trust. Keep the output directory across
releases. Copy packages before exposing the new catalog.

Submit a PR to the [Get Shared catalog](../catalog/README.md) to list an
extension. Users can install and receive updates through that listing. For
private or unlisted repositories, users can paste the feed's base URL under
**Extensions → Advanced → Add repository** and verify the public key before
browsing. The publisher association is saved when an extension is installed.

## Legacy TUF feeds

Existing TUF feeds use `scripts/extension-repository.py` and the legacy
`updates/systemd/manatan-extension-metadata.*` examples. Keep any published
metadata and targets available for clients that still use them. The new
mirror leaves those files alone. Root keys and legacy online keys are kept
outside the source and public feed. The official Actions release job signs
only the new catalog.
