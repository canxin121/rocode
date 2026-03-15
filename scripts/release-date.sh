#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_CARGO="$ROOT_DIR/Cargo.toml"
SYNC_SCRIPT="$ROOT_DIR/scripts/sync_version.sh"

INPUT_DATE="${1:-}"
if [[ -n "$INPUT_DATE" ]]; then
  if [[ ! "$INPUT_DATE" =~ ^([0-9]{4})-([0-9]{2})-([0-9]{2})$ ]]; then
    echo "Usage: $0 [YYYY-MM-DD]" >&2
    exit 1
  fi
  YEAR="${BASH_REMATCH[1]}"
  MONTH_RAW="${BASH_REMATCH[2]}"
  DAY_RAW="${BASH_REMATCH[3]}"
else
  YEAR="$(date +%Y)"
  MONTH_RAW="$(date +%m)"
  DAY_RAW="$(date +%d)"
fi

MONTH="$((10#$MONTH_RAW))"
DAY="$((10#$DAY_RAW))"
TARGET_VERSION="$YEAR.$MONTH.$DAY"

TARGET_VERSION="$TARGET_VERSION" perl -0pi -e '
  s/(\[workspace\.package\]\n(?:.*\n)*?version = ")\d{4}\.\d{1,2}\.\d{1,2}(")/${1}$ENV{TARGET_VERSION}${2}/s
    or die "Failed to update [workspace.package].version\n";
' "$ROOT_CARGO"

"$SYNC_SCRIPT"

echo "Released version $TARGET_VERSION"
