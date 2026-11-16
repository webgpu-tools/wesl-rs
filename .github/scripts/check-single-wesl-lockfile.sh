#!/usr/bin/env bash
# Fail if Cargo.lock contains more than one `wesl` package (multiple versions).
set -euo pipefail

lockfile="${1:-Cargo.lock}"

if [[ ! -f "$lockfile" ]]; then
  echo "error: lockfile not found: $lockfile" >&2
  exit 1
fi

mapfile -t versions < <(
  awk '
    /^name = "wesl"$/ {
      if (getline > 0 && $1 == "version") {
        gsub(/"/, "", $3)
        print $3
      }
    }
  ' "$lockfile"
)

count="${#versions[@]}"

if ((count == 0)); then
  echo "error: no \`wesl\` package entry found in $lockfile" >&2
  exit 1
fi

if ((count > 1)); then
  echo "error: $lockfile lists $count \`wesl\` packages (expected 1): ${versions[*]}" >&2
  exit 1
fi

echo "ok: single \`wesl\` package in $lockfile (version ${versions[0]})"