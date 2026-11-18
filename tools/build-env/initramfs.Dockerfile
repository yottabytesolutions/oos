FROM ubuntu:26.04@sha256:065aa8626841f16057b78b027b464cd59c4fac36c204c72dcbf48d66f5fa8a96
RUN sed -i 's/^Types: deb$/Types: deb deb-src/' /etc/apt/sources.list.d/ubuntu.sources \
    && apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends @PACKAGES@ \
    && mkdir -p /usr/src/busybox \
    && cd /usr/src/busybox \
    && apt-get source busybox=$(dpkg-query -W -f='${Version}' busybox-static) \
    && cd busybox-* \
    && cp debian/config/pkg/static .config \
    && sed -i -e 's/^CONFIG_TELNET=y/# CONFIG_TELNET is not set/' \
        -e 's/^CONFIG_TELNETD=y/# CONFIG_TELNETD is not set/' .config \
    && make oldconfig </dev/null \
    && make -j"$(nproc)" \
    && /bin/busybox --list | grep -Ev "^telnet(d)?$" > /tmp/expected-applets \
    && ./busybox --list > /tmp/actual-applets \
    && diff -u /tmp/expected-applets /tmp/actual-applets \
    && ! ./busybox --list | grep -Ex 'telnet(d)?' \
    && cp busybox /bin/busybox \
    && { dpkg-query -W -f='${Package} ${Version}\n' busybox-static; \
         printf 'Built-Using: glibc (= %s)\n' "$(dpkg-query -W -f='${source:Version}' libc6-dev)"; } \
         > /usr/local/share/oos-busybox.version \
    && rm -rf /var/lib/apt/lists/*
