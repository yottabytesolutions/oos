# ADR 0022: One SCPI interpreter in the frame loop; every remote transport only frames its messages

- Status: accepted
- Date: 2026-11-17
- Category: code (touches system: the network services, ADR 0024 and 0025)

## Context

A computer controls the scope in several ways: the raw SCPI socket (TCP 5555, the stock firmware's port), VXI-11
(what VISA uses for `TCPIP::<host>::INSTR`), HiSLIP and the `scpi` tool of the MCP endpoint of the web remote
(`POST /mcp`). Each carries the same kind of message: an SCPI program and its answer. What differs is the framing:
a newline, ONC RPC with XDR records, the HiSLIP header or JSON-RPC.

The commands change the scope's state (`Ui`, the model, the acquisition), which the frame loop owns. Until 0.1.6 the
servers read their sockets in the frame loop with a 1 ms timeout, which the kernel rounds up to 4 to 8 ms; with an
SCPI connection open the screen ran at 16 instead of 30 frames a second (verified on the unit: DHO924S over Ethernet,
CHANGELOG 0.1.6). Since then every server reads without waiting.

## Decision

- There is one SCPI interpreter (`ui/src/scpi.rs`, `Scpi::exec` and `Scpi::poll_program`). Every transport hands it
  the decoded program and sends back what it returns; no transport parses or answers an SCPI command itself.
- The interpreter runs in the frame loop, on the state the screen shows, without locks. The servers currently poll
  sockets from that loop. ADR 0037 plans their move into the IO layer: one async IO thread hands decoded programs
  to the frame loop over bounded channels and receives replies. That move is not implemented yet; it leaves the
  interpreter and its continuations in the frame loop.
- A query that has to wait (`*OPC?` on an armed Single, Autoset, a SIM run) is kept as a per-client continuation and
  answered on a later pass; the loop does not wait for it. A transport that cannot keep one (the synchronous MCP and
  `/cmd` adapters) refuses the barrier with error -221 instead of answering `1` too early.

## Alternatives

- **A parser per transport:** each would drift from the others, and a fix would have to be made for every transport.
- **The interpreter in a thread of its own, the state behind a lock:** a command would wait for the frame, the frame for
  the command, and every part of `Ui` that SCPI touches would need a lock. The frame loop already owns the state.
- **Blocking reads with a short timeout:** what the servers did before 0.1.6; it cost half the frame rate (see Context).

## Consequences

- A new command is written once and works on every transport; its reference is [scpi.md](../../scpi.md) (ADR 0023).
- A command that takes long stops the screen for that long. Long work (a deep record, a decode, a SIM run) goes to a
  worker, and the command reports BUSY or completes through `*OPC`.
- VXI-11's read path cannot wait for a pending answer: a read during a pending `*OPC?` returns timeout error 15 at once,
  without honouring the client's timeout. A VXI client retries; raw TCP waits for the answer
  ([scpi.md](../../scpi.md)).
- Real session semantics, such as locks between two controllers, would need a further decision.

## References

`ui/src/scpi.rs`, `ui/src/vxi11.rs`, `ui/src/hislip.rs`, `ui/src/http.rs` (`/mcp`), `ui/src/net.rs`, `ui/src/main.rs`
(`lan_servers`), [scpi.md](../../scpi.md) (Operation completion), CHANGELOG 0.1.6.
