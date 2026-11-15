FROM @BASE@
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends @PACKAGES@ && rm -rf /var/lib/apt/lists/*
