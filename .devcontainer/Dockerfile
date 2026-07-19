FROM mcr.microsoft.com/devcontainers/base:ubuntu24.04

ARG CMAKE_VERSION=4.4.0
ARG JUST_VERSION=1.50.0

# [Optional] Uncomment this section to install additional vcpkg ports.
# RUN su vscode -c "${VCPKG_ROOT}/vcpkg install <your-port-name-here>"

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
    build-essential ca-certificates clang-20 clang-format-20 clang-tidy-20 clang-tools-20 \
    clangd-20 curl libsqlite3-dev sqlite3

RUN case "$(uname -m)" in \
    x86_64) CMAKE_ARCH=x86_64 ;; \
    aarch64|arm64) CMAKE_ARCH=aarch64 ;; \
    *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;; \
    esac \
    && CMAKE_ARCHIVE="cmake-${CMAKE_VERSION}-linux-${CMAKE_ARCH}.tar.gz" \
    && curl --location --proto '=https' --tlsv1.2 --fail --silent --show-error \
    "https://github.com/Kitware/CMake/releases/download/v${CMAKE_VERSION}/${CMAKE_ARCHIVE}" \
    --output "/tmp/${CMAKE_ARCHIVE}" \
    && curl --location --proto '=https' --tlsv1.2 --fail --silent --show-error \
    "https://github.com/Kitware/CMake/releases/download/v${CMAKE_VERSION}/cmake-${CMAKE_VERSION}-SHA-256.txt" \
    --output /tmp/cmake-SHA-256.txt \
    && grep " ${CMAKE_ARCHIVE}$" /tmp/cmake-SHA-256.txt > /tmp/cmake-archive.sha256 \
    && cd /tmp \
    && sha256sum --check cmake-archive.sha256 \
    && tar --extract --gzip --file "${CMAKE_ARCHIVE}" --strip-components=1 --directory /usr/local \
    && rm "${CMAKE_ARCHIVE}" cmake-SHA-256.txt cmake-archive.sha256

RUN curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
    https://just.systems/install.sh --output /tmp/just-install.sh \
    && bash /tmp/just-install.sh --to /usr/local/bin --tag "${JUST_VERSION}" \
    && rm /tmp/just-install.sh
