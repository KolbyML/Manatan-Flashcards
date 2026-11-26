#!/usr/bin/env bash
set -euo pipefail

feed="$1"
packages="$2"
package="$3"
rollout="$4"
root="$(cd "$(dirname "$0")/.." && pwd)"

if [[ ! "$rollout" =~ ^(0|[1-9][0-9]?|100)$ ]]; then
  echo "Rollout must be an integer from 0 to 100." >&2
  exit 1
fi
if [[ -z "${PUBLISHER_PEM:-}" ]]; then
  echo "The protected release environment needs the official publisher key." >&2
  exit 1
fi
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
(umask 077; printf '%s\n' "$PUBLISHER_PEM" > "$scratch/publisher.pem")
unset PUBLISHER_PEM
mkdir "$scratch/packages"
case "$package" in
  all) cp -- "$packages"/*.mext "$scratch/packages/" ;;
  review-heatmap|anki-connect|reviewer-progress)
    cp -- "$packages/$package"-*.mext "$scratch/packages/" ;;
  *) echo "Unknown package selection." >&2; exit 1 ;;
esac
python3 "$root/scripts/publish-extension-feed.py" --key "$scratch/publisher.pem" \
  --out "$feed" --packages "$scratch/packages" --rollout "$rollout"
