#!/usr/bin/env bash
# Runs the checks and/or the end-to-end test in an Ubuntu 24.04 container, on a copy of the
# working tree (tracked and new files, nothing git ignores). Works with Docker on Linux, macOS,
# and Windows (Git Bash with Docker Desktop).
#
#   bash e2e/linux/run.sh checks   # scripts/ci/checks.sh
#   bash e2e/linux/run.sh e2e      # build and install the .deb, then drive it with WebDriver
#   bash e2e/linux/run.sh all      # both (the default)
#
# Screenshots, results.json and logs land in e2e/linux/out/.
set -euo pipefail
mode=${1:-all}
case "$mode" in checks | e2e | all) ;; *) echo "usage: $0 [checks|e2e|all]" >&2; exit 2 ;; esac

root=$(cd "$(dirname "$0")/../.." && pwd)
image=opencompanion-linux-e2e
out="$root/e2e/linux/out"
rm -rf "$out" && mkdir -p "$out"

docker build -t "$image" -f "$root/e2e/linux/Dockerfile" "$root/scripts/ci"

# Docker Desktop on Windows wants a Windows path for the bind mount.
mount=$out
if command -v cygpath >/dev/null 2>&1; then mount=$(cygpath -w "$out"); fi

# Named volumes keep the Rust build and the Bun cache between runs.
(cd "$root" && git ls-files -z --cached --others --exclude-standard | tar --null -cf - -T -) |
  MSYS_NO_PATHCONV=1 docker run --rm -i --shm-size=2g \
    -v opencompanion-e2e-cargo:/opt/cargo/registry \
    -v opencompanion-e2e-target:/work/src-tauri/target \
    -v opencompanion-e2e-bun:/root/.bun/install/cache \
    -v "$mount:/out" \
    "$image" bash -c "tar -xf - -C /work && bash /work/e2e/linux/inside.sh $mode"
