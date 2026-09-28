#!/usr/bin/env bash
# Runs inside the container started by run.sh, with the working tree in /work and /out mounted.
set -euo pipefail
mode=$1
cd /work

if [ "$mode" = checks ] || [ "$mode" = all ]; then
  bash scripts/ci/checks.sh 2>&1 | tee /out/checks.log
fi
[ "$mode" = checks ] && exit 0

bun install --frozen-lockfile
# Full output: a CI job with no output for too long is cancelled, and errors must stay visible.
bun run tauri build --bundles deb 2>&1 | tee /out/tauri-build.log
# Its dependencies (WebKitGTK, GTK, AppIndicator) came with the image.
dpkg -i ./src-tauri/target/release/bundle/deb/*.deb > /out/deb-install.log 2>&1
(cd src-tauri && cargo build --bin fake-cli)

# Stand-ins named like the real CLIs. They sit in a folder that only the login shell's PATH
# holds (~/.profile), not the PATH the app starts with, so finding and running them proves the
# app reads the login shell's environment. They run fake-cli by name for the same reason.
fakes=/root/.local/share/oc-fakes/bin
mkdir -p "$fakes"
cp src-tauri/target/debug/fake-cli "$fakes/fake-cli"
printf '#!/bin/sh\n[ "$1" = "--version" ] && { echo "2.1.99 (Claude Code)"; exit 0; }\nexec fake-cli "$@"\n' > "$fakes/claude"
printf '#!/bin/sh\n[ "$1" = "--version" ] && { echo "1.18.0"; exit 0; }\nexec fake-cli "$@"\n' > "$fakes/opencode"
chmod +x "$fakes"/*
echo "export PATH=\"$fakes:\$PATH\"" >> /root/.profile

rm -rf /root/.local/share/dev.opencompanion.app /root/.config/autostart /root/e2e-project
mkdir -p /root/e2e-project && echo '{}' > /root/e2e-project/package.json

Xvfb :99 -screen 0 1440x900x24 -nolisten tcp > /out/xvfb.log 2>&1 &
export DISPLAY=:99
for _ in $(seq 1 50); do xdpyinfo > /dev/null 2>&1 && break; sleep 0.1; done
eval "$(dbus-launch --sh-syntax)"
# A notification server, so notifications can be listed and clicked (dunstctl).
dunst > /out/dunst.log 2>&1 &
tauri-driver > /out/tauri-driver.log 2>&1 &
for _ in $(seq 1 50); do curl -s http://127.0.0.1:4444/status > /dev/null 2>&1 && break; sleep 0.2; done

OC_OUT=/out OC_PROJECT=/root/e2e-project python3 -u e2e/linux/e2e.py 2>&1 | tee /out/e2e.log
exit "${PIPESTATUS[0]}"
