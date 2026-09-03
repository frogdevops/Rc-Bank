FROM archlinux:base-devel

RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm \
    git \
    docker \
    docker-compose \
    rustup \
    clang \
    cmake \
    neovim

RUN useradd -m -s /bin/bash rc && \
    echo "rc ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/rc

USER rc
RUN rustup default stable