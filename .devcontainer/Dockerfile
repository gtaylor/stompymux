FROM mcr.microsoft.com/devcontainers/base:ubuntu24.04

LABEL org.opencontainers.image.source="https://github.com/gtaylor/stompymux"

# Bake the shared toolchain into the image. Environment-specific setup runs later
# from devcontainer.json's postCreateCommand via setup.sh.
COPY lib.sh install-tools.sh install-rust.sh /tmp/devcontainer/
RUN bash /tmp/devcontainer/install-tools.sh \
    && su vscode -c 'bash /tmp/devcontainer/install-rust.sh' \
    && rm -rf /tmp/devcontainer

USER vscode
ENV PATH="/home/vscode/.cargo/bin:${PATH}"
