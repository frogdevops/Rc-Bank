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
RUN groupadd -g 800 docker && \
    useradd -m -s /bin/bash -G docker rc && \
    echo "rc ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/rc
USER rc
RUN rustup default stable