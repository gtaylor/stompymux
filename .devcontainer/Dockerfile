FROM mcr.microsoft.com/devcontainers/base:ubuntu24.04

ARG CMAKE_VERSION=4.4.2
ARG JUST_VERSION=1.57.0
ARG STYLUA_VERSION=2.5.2

# [Optional] Uncomment this section to install additional vcpkg ports.
# RUN su vscode -c "${VCPKG_ROOT}/vcpkg install <your-port-name-here>"

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
    build-essential ca-certificates clang-20 clang-format-20 clang-tidy-20 clang-tools-20 \
    clangd-20 curl libsqlite3-dev sqlite3 unzip

RUN CMAKE_ARCHIVE="cmake-${CMAKE_VERSION}-linux-x86_64.tar.gz" \
    && curl --location --proto '=https' --tlsv1.2 --fail --silent --show-error \
    "https://github.com/Kitware/CMake/releases/download/v${CMAKE_VERSION}/${CMAKE_ARCHIVE}" \
    --output "/tmp/${CMAKE_ARCHIVE}" \
    && tar --extract --gzip --file "/tmp/${CMAKE_ARCHIVE}" --strip-components=1 --directory /usr/local \
    && rm "/tmp/${CMAKE_ARCHIVE}"

RUN curl --proto '=https' --tlsv1.2 --fail --silent --show-error \
    https://just.systems/install.sh --output /tmp/just-install.sh \
    && bash /tmp/just-install.sh --to /usr/local/bin --tag "${JUST_VERSION}" \
    && rm /tmp/just-install.sh

RUN STYLUA_ARCHIVE="stylua-linux-x86_64.zip" \
    && curl --location --proto '=https' --tlsv1.2 --fail --silent --show-error \
    "https://github.com/JohnnyMorganz/StyLua/releases/download/v${STYLUA_VERSION}/${STYLUA_ARCHIVE}" \
    --output "/tmp/${STYLUA_ARCHIVE}" \
    && unzip -q "/tmp/${STYLUA_ARCHIVE}" -d /usr/local/bin \
    && rm "/tmp/${STYLUA_ARCHIVE}"
