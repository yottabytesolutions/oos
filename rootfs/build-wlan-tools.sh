#!/bin/sh
# Copyright (C) 2026 Julian Decker
# SPDX-License-Identifier: GPL-2.0-or-later
# build-wlan-tools.sh - static wpa_supplicant and wpa_cli for the image (docs/wlan.md), in Docker, into
# rootfs/out/. boot/build-dev-image.sh runs it when they are missing. Only what WPA/WPA2 personal needs: nl80211,
# the control socket, the internal crypto. No EAP, no SAE, no P2P.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
out="$here/out"
root=$(cd "$here/.." && pwd)
image=$("$root/tools/build-env.sh" prepare rootfs-wlan-tools)
before=$(python3 "$root/tools/output.py" identity "$out/wpa_supplicant")
. "$here/deps.sh"
ver=$(dep_ver wpa_supplicant)
dep_fetch wpa_supplicant >/dev/null
mkdir -p "$out"
# libnl comes from Alpine's libnl3-static and is linked in (LGPL-2.1-or-later): its source goes with a release
# (THIRD-PARTY-LICENSES.md). Alpine builds the package from these two files; the build stops below when the package
# Alpine installs is another version than the one fetched here (then rootfs/deps.lock follows).
libnl=$(dep_ver libnl)
dep_fetch libnl >/dev/null
dep_fetch libnl-musl-patch >/dev/null
# arm64 containers: on an x86-64 host through QEMU (boot/build-dev-image.sh says how); the results belong to the caller
docker run --rm --pull=never --platform linux/arm64 -e OUT_OWNER="$(id -u):$(id -g)" -e VER="$ver" -e LIBNL="$libnl" -v "$out:/out" -v "$DEPS_SOURCES:/src:ro" "$image" sh -euc '
	apk list -I libnl3-static 2>/dev/null | grep -q "^libnl3-static-$LIBNL-r" || { echo "libnl3-static of this Alpine is not $LIBNL (rootfs/deps.lock): $(apk list -I libnl3-static 2>/dev/null)" >&2; exit 1; }
	cd /tmp && tar -xzf /src/wpa_supplicant-$VER.tar.gz && cd wpa_supplicant-$VER/wpa_supplicant
	# Distinguish parser rejection from I/O and interface failures at startup.
	# A failed serialization must leave the previous RAM copy intact and make SAVE_CONFIG fail.
	# Encode context prefixes so the shell source has no trailing whitespace.
	sed "s/^|/ /" <<PATCH | patch -p0
--- config_file.c
+++ config_file.c
@@ -302,6 +302,7 @@
|	FILE *f;
|	char buf[512], *pos;
|	int errors = 0, line = 0;
+	int read_error;
|	struct wpa_ssid *ssid, *tail, *head;
|	struct wpa_cred *cred, *cred_tail, *cred_head;
|	struct wpa_config *config;
@@ -391,14 +392,24 @@
|		}
|	}
|
-	fclose(f);
+	read_error = ferror(f);
+	if (fclose(f))
+		read_error = 1;
|
|	config->ssid = head;
|	wpa_config_debug_dump_networks(config);
|	config->cred = cred_head;
|
+	if (read_error) {
+		wpa_printf(MSG_ERROR, "Could not read configuration file.");
+		if (config != cfgp)
+			wpa_config_free(config);
+		return NULL;
+	}
+
|#ifndef WPA_IGNORE_CONFIG_ERRORS
|	if (errors) {
+		wpa_printf(MSG_ERROR, "Configuration file rejected by parser.");
|		if (config != cfgp)
|			wpa_config_free(config);
|		config = NULL;
@@ -1653,10 +1664,10 @@
|
|	tmp_len = os_strlen(name) + 5; /* allow space for .tmp suffix */
|	tmp_name = os_malloc(tmp_len);
-	if (tmp_name) {
-		os_snprintf(tmp_name, tmp_len, "%s.tmp", name);
-		name = tmp_name;
-	}
+	if (!tmp_name)
+		return -1;
+	os_snprintf(tmp_name, tmp_len, "%s.tmp", name);
+	name = tmp_name;
|
|	wpa_printf(MSG_DEBUG, "Writing configuration file '%s'", name);
|
@@ -1700,9 +1711,10 @@
|	}
|#endif /* CONFIG_NO_CONFIG_BLOBS */
|
-	os_fdatasync(f);
-
-	fclose(f);
+	if (ferror(f) || os_fdatasync(f))
+		ret = -1;
+	if (fclose(f))
+		ret = -1;
|
|	if (tmp_name) {
|		int chmod_ret = 0;
@@ -1711,7 +1723,7 @@
|		chmod_ret = chmod(tmp_name,
|				  S_IRUSR | S_IWUSR | S_IRGRP | S_IWGRP);
|#endif /* ANDROID */
-		if (chmod_ret != 0 || rename(tmp_name, orig_name) != 0)
+		if (!ret && (chmod_ret != 0 || rename(tmp_name, orig_name) != 0))
|			ret = -1;
|
|		os_free(tmp_name);
PATCH
	cat > .config <<CFG
CONFIG_DRIVER_NL80211=y
CONFIG_LIBNL32=y
CONFIG_CTRL_IFACE=y
CONFIG_BACKEND=file
CONFIG_TLS=internal
CONFIG_INTERNAL_LIBTOMMATH=y
CONFIG_NO_RANDOM_POOL=y
CONFIG_GETRANDOM=y
CONFIG_IEEE80211W=y
CONFIG_NO_CONFIG_BLOBS=y
CONFIG_DEBUG_FILE=y
CFG
	export LDFLAGS=-static CFLAGS="-Os -ffunction-sections -fdata-sections"
	make -j"$(nproc)" wpa_supplicant wpa_cli >/tmp/build.log 2>&1 || { tail -30 /tmp/build.log; exit 1; }
	strip wpa_supplicant wpa_cli
	cp wpa_supplicant wpa_cli /out/
	# the licence text itself stands in the README, up to the end of its disclaimer
	sed "/SUCH DAMAGE/q" ../README > /out/wpa_supplicant.COPYING
	chown "$OUT_OWNER" /out/*
'
ls -la "$out/wpa_supplicant" "$out/wpa_cli"

"$root/tools/build-env.sh" provenance rootfs-wlan-tools --image "$image" --output "$out/wlan-tools.packages.txt"

python3 "$root/tools/output.py" stamp "$out/wpa_supplicant" "$before"
python3 "$root/tools/output.py" stamp "$out/wpa_cli" "$before"
