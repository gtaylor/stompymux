FROM mcr.microsoft.com/devcontainers/base:ubuntu24.04

LABEL org.opencontainers.image.source="https://github.com/gtaylor/stompymux"

COPY install-tools.sh /tmp/install-tools.sh
RUN bash /tmp/install-tools.sh && rm /tmp/install-tools.sh
