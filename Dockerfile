
FROM rust:1-alpine3.20

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

RUN cargo install cargo-appimage

# appimagetool needs FUSE to mount the AppImage; containers usually can't
# provide that, so fall back to extract-and-run.
ENV APPIMAGE_EXTRACT_AND_RUN=1

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

CMD ["cargo", "appimage"]