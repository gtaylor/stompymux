FROM mcr.microsoft.com/devcontainers/base:ubuntu24.04

ARG CMAKE_VERSION=4.4.2
ARG JUST_VERSION=1.57.0
ARG STYLUA_VERSION=2.5.2

# [Optional] Uncomment this section to install additional vcpkg ports.
# RUN su vscode -c "${VCPKG_ROOT}/vcpkg install <your-port-name-here>"

COPY install-tools.sh /tmp/install-tools.sh
RUN bash /tmp/install-tools.sh && rm /tmp/install-tools.sh
