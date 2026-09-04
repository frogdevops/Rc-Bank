FROM archlinux:base-devel
RUN pacman -Syu --noconfirm && \
    pacman -S --noconfirm \
    git \
    docker \
    docker-compose \
    rustup \
    clang \
    cmake \
    github-cli \
    openssh \
    neovim

RUN groupmod -g 800 docker && \
    useradd -m -s /bin/bash -G docker rc && \
    echo "rc ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/rc && \
    ssh-keygen -A

USER rc
RUN rustup default stable