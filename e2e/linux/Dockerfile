# Ubuntu 24.04 with everything OpenCompanion needs to build, test and run under a virtual display.
# Built by e2e/linux/run.sh with scripts/ci as the context.
FROM ubuntu:24.04

ENV LANG=C.UTF-8 \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    BUN_INSTALL=/opt/bun \
    PATH=/opt/cargo/bin:/opt/bun/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    SHELL=/bin/bash

COPY linux-deps.sh /tmp/linux-deps.sh
RUN bash /tmp/linux-deps.sh && rm /tmp/linux-deps.sh

WORKDIR /work
