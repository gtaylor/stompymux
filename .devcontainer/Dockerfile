FROM mcr.microsoft.com/devcontainers/base:ubuntu24.04

LABEL org.opencontainers.image.source="https://github.com/gtaylor/stompymux"

COPY install-tools.sh /tmp/install-tools.sh
RUN bash /tmp/install-tools.sh && rm /tmp/install-tools.sh

USER vscode
RUN curl --location --proto '=https' --tlsv1.2 --fail --silent --show-error \
      https://sh.rustup.rs --output /tmp/rustup-init.sh \
    && sh /tmp/rustup-init.sh -y --profile minimal --component rustfmt,clippy \
    && rm /tmp/rustup-init.sh

ENV PATH="/home/vscode/.cargo/bin:${PATH}"
