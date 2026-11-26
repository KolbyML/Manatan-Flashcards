# Get Shared catalog

The app reads `catalog/extensions.json` from the protected `catalog` branch.
Submit a pull request against that branch for a new listing, a description or
source URL change, or a publisher key change. One publisher can list several
extensions using the same publisher entry. Maintainers review the listing and
publisher key before merging.

Copy an existing listing and supply a unique extension ID, its display name
and description, the source repository URL, and the publisher's own HTTPS
feed URLs and Ed25519 public key. Run
`python3 scripts/check-extension-catalog.py catalog/extensions.json`
before submitting. Publish an installable signed release before requesting
review. See [publisher setup](../updates/README.md#third-party-feeds).

Publish new versions to your own feed. Routine releases require no catalog
change. The app verifies signed releases on install and update and asks the
user to approve the package's permissions. Removing a listing only removes
it from discovery. Installed extensions keep their current publisher binding.

## Launch

Initialize `catalog` from the reviewed source containing this file and the
catalog validation workflow. Protect it with a required PR, a required
`Validate catalog` check, and no direct-push bypasses. `master` remains the
development branch. The app's default catalog address is:

`https://raw.githubusercontent.com/KolbyML/Manatan-Flashcards/catalog/catalog/extensions.json`
