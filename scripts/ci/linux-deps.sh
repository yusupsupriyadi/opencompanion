#!/usr/bin/env bash
# Installs what building, testing and driving OpenCompanion on Ubuntu 24.04 needs: the Tauri 2
# prerequisites, WebKitWebDriver, a virtual display, Node.js (Vitest runs on Node), Bun, Rust
# and tauri-driver. Used by e2e/linux/Dockerfile, so a local run and CI install the same things.
set -euo pipefail

# jsdom wants ^24.15.0 on Node 24; .circleci/config.yml pins the same version.
NODE_VERSION=24.21.0
export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y --no-install-recommends \
  build-essential curl wget file unzip git ca-certificates pkg-config xz-utils \
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  webkit2gtk-driver xvfb xauth dbus-x11 at-spi2-core x11-utils xclip xdg-utils dunst \
  python3 procps fonts-dejavu-core
rm -rf /var/lib/apt/lists/*

curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" -o /tmp/node.tar.xz
tar -xf /tmp/node.tar.xz -C /opt
ln -sf "/opt/node-v${NODE_VERSION}-linux-x64/bin/node" /usr/local/bin/node
ln -sf "/opt/node-v${NODE_VERSION}-linux-x64/bin/npm" /usr/local/bin/npm
rm /tmp/node.tar.xz

curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable -c clippy
curl -fsSL https://bun.sh/install | bash
cargo install tauri-driver --locked
