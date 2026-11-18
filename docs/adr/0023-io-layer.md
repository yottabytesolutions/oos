# Non-blocking IO stays behind ports in one IO layer

- Number: 0023
- Category: code
- State: target: M15b and IO-port follow-up in docs/architecture/migration.md#adr-baseline-follow-ups
- Date: 2026-11-18

## Context

Frame-thread socket writes could stall for 2 seconds, with a 30-second total limit. Verified on the unit: DHO804,
2026-11-18, M15a sustained at least 26 fps under a stalled client versus 0.2 fps on main, and 628-1038 exchanges/s
versus 30-34. The accepted host cost was +685 production LOC, +2.86% ARM binary size and 9 dependencies ([task
0016](../tasks/0016-m15-remote-adapters.md#measurements)).

## Decision

Use tokio for network IO: raw SCPI, VXI-11, HiSLIP, HTTP, mDNS, NTP, update checks. USB bulk or WLAN control moves
qualify when they remove a blocking call from the frame thread or remove a dedicated thread. Use one current-thread IO
runtime in netio. Clean ports take priority over executor choice. Tokio types never cross the port; synchronous owned
values and bounded channels do. The frame thread never awaits or waits on sockets; it drains its port and is woken by
eventfd. Send timers close after 2 seconds without progress or 30 seconds total.

Synchronous exceptions are:

- Core and DSP computations: no waiting IO.
- Pure HAL plans: no waiting IO.
- Hardware register sequences behind Board: ordered operations and required hardware gaps.

USB/WLAN moves must demonstrate the criterion above. Other synchronous IO exceptions require a reason in this record.

Check: `architecture` confines tokio to netio; transport timer/cancellation tests. A gate rule excluding std::net from
non-IO layers is a target follow-up.

## Alternatives

One poll(2) IO thread: rejected because it retains handwritten timers and per-protocol state machines. A thread per
connection: rejected because threads and duplicated accept loops grow with clients. Async throughout: rejected because
the CPU-bound frame thread gains latency and no useful waiting concurrency.

## Consequences

Raw SCPI, VXI-11 and HiSLIP use netio today. HTTP, mDNS, NTP, update checks and device-stream moves meeting the
criterion above remain targets.
