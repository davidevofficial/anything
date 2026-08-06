FROM rust:1-bullseye AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libgl1-mesa-dev \
    libx11-dev \
    libxcursor-dev \
    libxi-dev \
    libxrandr-dev \
    libxkbcommon-dev \
    fuse \
    file \
    git \
    curl \
    squashfs-tools \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-appimage

ENV APPIMAGE_EXTRACT_AND_RUN=1
RUN curl -L --retry 5 --retry-delay 2 --retry-connrefused \
        -o /usr/local/bin/appimagetool \
        "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" \
    && chmod +x /usr/local/bin/appimagetool

WORKDIR /app

# Cache dependencies separately from source
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Now copy real source — only this layer rebuilds on code changes
COPY . .
RUN touch src/main.rs && cargo appimage