# GPU drawing uses a helper and the CPU path stays complete

- Number: 0026
- Category: code
- State: in force
- Date: 2026-11-18

## Context

Verified on the unit: DHO924S, 2026-11-12, command drawing reduced frame-loop CPU from 53% to 35%. The Rust scope binary
uses static musl; Mesa needs glibc. In the same measurement the helper used 8.5% of a core; uploading CPU-drawn pixel
layers had cost 44% frame-loop CPU. This supports command recording instead of pixel upload or in-process Mesa.

## Decision

With GPU graphics, the CPU records under/over-trace drawing commands and phosphor maps. The separate C oos-gpu helper
links glibc and Mesa, draws those commands and scales monitor output. It shares a fixed memory arena and socket protocol
with oos-ui. Any helper exit, error reply or timeout returns drawing to the CPU. The CPU renderer stays complete. Every
drawing primitive has a helper command, shader case and replay test.

Check: CPU command-replay equality tests, gpu/test-host.sh and device-test frame_rate_by_graphics_and_monitor.

## Alternatives

Upload CPU-drawn layers: rejected after it used 44% frame-loop CPU versus 35% with commands. Link Mesa in-process:
rejected because it requires glibc and removes GPU failure isolation from static musl Rust.

## Consequences

Failure isolation costs an IPC protocol and a second language/libc. Linking Mesa into the Rust program would remove that
isolation. GPU cost follows covered area. Verified on the unit: DHO924S, 2026-11-15, avoiding repeated border fills
reduced HDMI Layout GPU time with the measurement page open from 38.5 to 12.3 ms.
