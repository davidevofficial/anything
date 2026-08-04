
FROM rust:1-alpine3.20

# musl-static means the resulting binary has ZERO glibc dependency,
# sidestepping the whole "which glibc version does the target machine have"
# problem entirely, rather than just picking an old-enough one.
RUN apk add --no-cache \
    musl-dev \
    build-base \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    mesa-dev \
    libx11-dev \
    libxcursor-dev \
    libxi-dev \
    libxrandr-dev \
    libxkbcommon-dev \
    fuse \
    file \
    git \
    curl \
    squashfs-tools

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo install cargo-appimage

# appimagetool needs FUSE to mount the AppImage; containers usually can't
# provide that, so fall back to extract-and-run.
ENV APPIMAGE_EXTRACT_AND_RUN=1

# The official AppImage/appimagetool project switched to shipping ONLY
# static binaries as of v1.9.0 specifically to fix cross-libc portability
# (see https://github.com/AppImage/appimagetool/pull/12) — this keeps the
# exact CLI cargo-appimage expects (unlike go-appimage's reimplementation,
# which parses arguments differently and broke here), while having zero
# glibc dependency so it runs fine on Alpine/musl.
RUN url=$(curl -s --retry 5 --retry-delay 2 --retry-connrefused \
        https://api.github.com/repos/AppImage/appimagetool/releases/tags/continuous \
        | grep "browser_download_url.*appimagetool-x86_64.AppImage\"" \
        | head -n1 \
        | cut -d '"' -f4) \
    && curl -L --retry 5 --retry-delay 2 --retry-connrefused \
        -o /usr/local/bin/appimagetool "$url" \
    && chmod +x /usr/local/bin/appimagetool

WORKDIR /app
COPY . .

# Build fully static, then package it. cargo-appimage picks up
# CARGO_BUILD_TARGET and points itself at the right target dir.
ENV CARGO_BUILD_TARGET=x86_64-unknown-linux-musl
CMD ["cargo", "appimage"]