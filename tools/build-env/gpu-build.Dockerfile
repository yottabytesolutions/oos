FROM @BASE@
RUN sed -i 's/^Types: deb$/Types: deb deb-src/' /etc/apt/sources.list.d/ubuntu.sources
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends @PACKAGES@ && rm -rf /var/lib/apt/lists/*
