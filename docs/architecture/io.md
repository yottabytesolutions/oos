# IO layer design

Issue #222, part of #220. The audit describes the 0.2.1 source. The design sections describe the planned
IO replacement under [ADR 0023](../adr/0023-io-layer.md), superseding the plumbing recipes in M15a-0, M15a
and M15b. The replacement keeps the existing SCPI interpreter. Hardware evidence below comes from M15a.

## Requirements audit

Carry forward the client-visible behaviours marked **keep**. Replace the machinery marked **drop** during cutover.

| 0.2.1 behaviour and evidence | Replacement requirements |
|---|---|
| Device intake passes one `Scpi` to raw TCP, VXI-11, HiSLIP and HTTP/MCP (`Remote::poll` in `ui/src/system/device/remote.rs`). The two-client fixture covers shared waveform selection, error consumption and ESR clearing (`ui/src/scpi/netio_tests.rs`). | **Keep.** Clients control one instrument. Its interpreter, program continuations, OPC fences and error state stay on the frame thread. |
| Instrument TCP sessions hold a `Credit` through output completion (`ui/src/netio/session.rs`, `ui/src/netio/tests.rs`). | **Keep** one outstanding exchange per connection for order and bounded memory. **Drop** `Credit` and its queue-cancellation bookkeeping. A sequential connection task gives the same bound. Accepted VXI-11 writes are the exception described below. |
| Raw intake reserves available bytes before taking a program, retains allocated output on yield, and can expand the reservation before resuming (`ui/src/scpi/intake.rs`). `Output` charges chunk and container capacity (`ui/src/netport/output.rs`). | **Keep** admission before side effects and execution at most once. **Drop** `Budget`, `Reservation`, `reserve_available` and reservation resizing. |
| Raw replies share one 64 MiB allocation budget (`RESPONSE_BYTES` in `ui/src/netport.rs`, `all_clients_share_one_response_budget_and_disconnect_releases_it`). VXI-11, HiSLIP and HTTP each have a separate budget (`ui/src/netio/mod.rs`, `ui/src/netport/http.rs`). | **Keep** the 64 MiB raw allocation ceiling and the 64 MiB logical SCPI response ceiling. **Drop** per-transport reservation formulae. Use the shared response windows below. |
| A full reply port parks an owned reply (`Ports::flush` in `ui/src/netport.rs`, `a_full_output_port_parks_owned_bytes_while_frames_and_input_advance`, `ui/src/hislip/ports/tests.rs`). | **Drop** parking and retry queues. **Keep** eventual delivery or cancellation, with each setter applied at most once and no frame wait. A oneshot has space for its one completion. |
| Output expires after 2 s without progress or 30 s total. Both clocks start when output begins. Positive writes renew only the idle deadline (`ui/src/netio/timer.rs`, `ui/src/netio/output.rs`). | **Keep.** Abandoned transfers must release their resources. |
| Raw intake visits the waiting programs already present at the start of the pass before taking new ones in FIFO order. A pending program yields to another connection. HiSLIP also gives all eight pending connections a turn (`ui/src/scpi/intake.rs`, `new_raw_client_gets_a_query_reply_during_single_opc`, `all_eight_pending_connections_get_an_intake_turn`). | **Keep** FIFO starts, command order and saved program selection across yields. **Drop** separate transport intake loops. Pending fences leave other clients and HTTP free to progress. |
| Raw and HiSLIP cap intake at eight turns and check a 0.5 ms time budget between turns. VXI-11 caps its RPC loop the same way, then separately advances link programs and pending display reads (`ui/src/scpi/intake.rs`, `ui/src/hislip/ports.rs`, `ui/src/vxi11/ports.rs`). | **Keep** these initial limits in the unified drain. A turn runs to completion. Measure the worst indivisible turn before rollout. |
| `remote.enqueue` reports enqueue latency p50/p99 from up to 256 samples younger than 10 s (`ui/src/netport/latency.rs`, `system_state_exposes_server_side_enqueue_quantiles`). | **Keep** its meaning: complete input message to successful enqueue, excluding idle socket time. |
| Raw waveform source and format survive an OPC yield and an intervening client (`waveform_source_and_format_are_atomic_across_a_continuation` in `ui/src/scpi/netio_tests.rs`). | **Keep.** The client receives the source and format it selected. Release interpreter ownership before transmission. |
| `*OPC?` suspends the program with its earlier replies and remaining commands. A cancelled operation clears the pending reply and tail and records `-410` (`Scpi::advance_commands` in `ui/src/scpi/message.rs`, `compatibility_cancelled_single_interrupts_query_and_does_not_follow_new_arm`). | **Keep** operation identity, once-only setters and the existing `*OPC`, `*CLS` and ESR semantics. A new SINGLE cannot satisfy an old fence. |
| Queries settle earlier setters and message completion settles remaining writes. Programs cap text at 4 MiB and commands at 1024. Normal error insertion caps the queue at 20 and truncates text at a UTF-8 boundary within 256 bytes (`Program::new`, `Scpi::push_err`, `Program::before` and `Program::settle`). | **Keep** parser syntax, errors, separators, binary blocks and setup effects. |
| An empty raw or VXI-11 query has a terminator. A write has no SCPI response (`raw_tcp_empty_query_is_a_line_and_compound_query_keeps_its_position`, `empty_query_has_a_readable_end_marker_and_cancelled_fence_has_no_reply`). | **Keep** the wire distinction. Complete the oneshot even for a no-reply command. |
| On raw TCP, read EOF permits a completed reply. A pending OPC is abandoned and closes the session before it can admit a buffered command. A complete HTTP request can still receive its response after read EOF (`ui/src/netio/session.rs`, `ui/src/scpi/intake.rs`, `ui/src/netio/http.rs`, `compatibility_disconnected_raw_client_cannot_resume_trailing_writes`). | **Keep** the transport-specific half-close rules, including discarding buffered raw commands on pending-client disconnect. Reset, service stop and timeout cancel the exchange. |
| Instrument listeners check `net.allow` on accept and recheck held peers on policy change. HTTP checks `POST /update` and `/mcp` before body admission and enforces browser origin policy (`ui/src/netio/runtime.rs`, `ui/src/netio/session.rs`, `ui/src/netport/http/head.rs`). | **Keep** live instrument ACL changes, empty-list semantics, IPv4/IPv6 and HTTP route policy, including access to public static routes. |
| Each instrument service has eight TCP slots. VXI-11 shares its eight across core and TCP portmapper connections and has a ninth request slot for UDP. HiSLIP counts both channels against its eight (`ui/src/netio/runtime.rs`, `rpc_request_queue_reserves_a_udp_slot_beside_the_eight_tcp_slots`). | **Keep** connection limits and UDP availability alongside eight TCP clients. **Drop** the ninth frame queue slot in favour of a bounded UDP actor. |
| The raw framing cap is 4 MiB, the RPC record limit is 1 MiB + 4096 bytes and the HiSLIP payload limit is 4 MiB. Host ports are assigned independently on loopback. Device listeners can use IPv6 with IPv4 fallback (`ui/src/netio/session.rs`, `ui/src/netport/rpc.rs`, `ui/src/netport/hislip.rs`, `host_listeners_are_private_and_independent`). | **Keep** ports, host isolation, framing bounds, fragmentation, coalescing and over-limit close/error results. |
| VXI-11 admits 16 server-wide links, with at most 16 pending programs and `MAX_RECORD` queued text bytes per link. Clear discards queued work and output. Destroy and owner disconnect release link state. Reads report END, REQCNT, timeout 15 or invalid link 4. An empty read normally times out immediately. With a nonzero RPC timeout and a pending display capture, an empty read can wait for pixels (`ui/src/vxi11.rs`, `ui/src/vxi11/ports.rs`). | **Keep** separate write and read exchanges, program order, terminators and cleanup. |
| VXI-11 appends before checking the output limit, so unread output can approach twice `RESP_MAX`. A read copies bytes from the link into an XDR reply (`pump_link` and `core_async` in `ui/src/vxi11.rs`, `a_read_larger_than_one_program_keeps_the_existing_payload_limit`). The full-output path can also discard accepted pending programs. | **Keep** reads spanning replies, including reads over 64 MiB. **Drop** append-then-check and duplicate buffers. Reserve resources before acknowledgement and retain accepted work through later reads. |
| VXI-11 GETPORT works on TCP and UDP port 111 on the device and advertises the selected core port. UDP uses a 1500-byte buffer (`ui/src/netio/rpc.rs`, `getport_uses_the_supplied_port_instead_of_the_device_default`). | **Keep** both transports, XDR padding, RPC record fragments and malformed-call results. |
| HiSLIP pairs sync and async channels, negotiates payload size, fragments output and acknowledges clear. Lock/remote/local requests acknowledge without implementing locking or overlap (`ui/src/hislip.rs`, `ui/src/netio/tests.rs`). | **Keep** that subset, including clear discarding the old program and its buffered bytes while fresh commands survive (`hislip_clear_discards_only_bytes_buffered_before_the_next_read`). |
| Device HiSLIP defaults off and mDNS defaults on. Disabling HiSLIP closes its peers and removes its advertisement (`ui/src/sysconf/instruments.rs`, `ui/src/sysconf.rs`, `ui/src/system/device/remote.rs`). | **Keep** device opt-in until unit verification and independence from other services. |
| mdns-sd owns packet handling, probing, conflicts and interface refresh. Netio handles registration, status, retry and shutdown (`ui/src/netio/mdns.rs`, `ui/src/netio/mdns/tests/events.rs`). | **Keep** active-service advertisements, withdrawal on off, conflict names, status and retry. Reuse the daemon. |
| HTTP allows eight active readers, eight screenshot waiters, two large senders and 18 sessions. It uses a 16 KiB head threshold while seeking the delimiter, 1 MiB for other bodies, 128 MiB for waveform imports and 70 MiB for updates. A delimiter found in the last read currently bypasses the threshold check (`ui/src/netport/http.rs`, `ui/src/netport/http/head.rs`, `ui/src/netio/http/input.rs`). | **Keep** resource limits and 431/413/503 results, with one request per connection. Enforce the head bound even when its delimiter arrives in the same read. **Drop** custom wakers and large-sender counters. |
| HTTP enforces a 10 s idle read, 120 s total read and 10 s until frame take. Early refusal and Continue writes have a 1 s limit (`ui/src/netport/http/head.rs`, `ui/src/netio/http/input.rs`, `ui/src/netio/http.rs`). | **Keep** 408/503 and rejection before Continue and bulk body admission. |
| HTTP admits one body over 1 MiB at a time through frame take. The upload worker owns the bytes, but the reader drops the large-body admission before worker cleanup. The frame receives the bounded header and job, checks the signed header and target, and starts full verification on the worker (`ui/src/netio/http.rs`, `ui/src/netport/http/upload.rs`, `Ui::update_upload` in `ui/src/ui/update.rs`). | **Keep** the two-stage checks, explicit model override and signed install lifecycle. Extend admission through the life of the bulk allocation, with no image hashing or installation on the frame thread. |
| App routes use frame state, while static assets bypass intake. The live-picture encoder has bounded ownership. Explicit HTTP and MCP display captures wait for readback when available, then encode on frame. HTTP `/cmd` checks `cmd::remote_ok`; the local FIFO has its own syntax and privileges (`ui/src/http.rs`, `ui/src/netport/http.rs`, `ui/src/netport/tests.rs`). | **Keep** the routes, coherent snapshots, live-picture worker and privilege check. Use the bounded encoding path for explicit capture results. |
| MCP uses the shared interpreter and JSON-RPC. Pending OPC returns a tool error. Display capture can defer an HTTP response while it waits for pixels (`Scpi::poll_message` in `ui/src/scpi/message.rs`, `ui/src/http/screen.rs`, `mcp_pending_opc_is_an_explicit_tool_error_not_success`). | **Keep** deferred display capture, the immediate OPC error and the distinction between empty query text and write success. |
| WLAN scan, join, forget and status run on a worker. Join has a 30 s timeout, idle status polls every 2 s, and successful joins and forgets save and sync the configuration. A failed new join is removed (`ui/src/ui/wlan.rs`). | **Keep** results, timeout and persistence. **Drop** sleeping worker loops and unbounded job/report channels when #219 is redone. |
| Bode USBTMC uses synchronous bulk transfers on its generator worker (`ui/src/usbtmc.rs`, `ui/src/ui/bode.rs`, ADR 0023). | **Keep** the synchronous worker and current USB scope. |

The performance floor comes from the [M15a unit measurements](../tasks/0016-m15-remote-adapters.md#measurements)
on 2026-11-18: at least 26 fps with a stalled reader. The owner's acceptance reference for #222 is 36 ms for
the other client's wait. The unit test for the replacement must demonstrate progress of frames, other clients
and HTTP while one client stalls or waits on OPC.

## Owners and messages

The remaining sections specify the target.

Use one current-thread tokio runtime on the IO thread. Each TCP connection has one task that owns its socket,
framing buffer and output cursor. Raw SCPI, HTTP and HiSLIP sync tasks read one exchange, await admission,
submit one typed request, await its oneshot and finish output before taking another exchange.
VXI-11 instead finishes one RPC exchange at a time; an acknowledged write transfers its program to a link actor
and does not wait for interpretation or a later device_read. A connection can therefore own several accepted
link programs, under the per-link limits below. Already-read pipelined bytes stay within the framing cap.
There is no credit object, reply router or socket on the frame thread.

One bounded `mpsc<Request>` leads to the frame thread. Its capacity is 32 requests. Raw SCPI, HTTP and HiSLIP
have at most one queued or active request per connection/session. VXI-11 has at most one per link, including
its pending continuation, across at most 16 links server-wide, not 16 per TCP connection. A link may additionally
have one side-effect-free preflight request for an unacknowledged write. All use this same queue.
Accepted VXI-11 programs waiting behind that link's submitted program stay in its bounded actor state.
An HTTP request contains a parsed operation and bounded values, not a raw socket or a generic HTTP request.
RPC headers, HiSLIP headers and discovery packets do not go through the SCPI interpreter.
The frame side holds one `Scpi`, bounded per-client/link continuations and the application handlers.
Pending storage is separate from the 32 queue slots: at most eight raw, eight HiSLIP and sixteen VXI-11
SCPI continuations (32 total), with at most one per connection/session or link. HTTP retains no OPC continuation.
Its existing eight screenshot positions bound deferred display work, including MCP SCPI waiting for pixels.

These are core signatures, not a second implementation. `ProgramText`, `PreparedProgram`, `WebRequest`, `ReplyBody`
and `FrameClock` name owned application values or existing seams. Their payload bounds are those audited above.
`ReplyBody` is an inline result or an owned, bounded encoding stream, with an explicit no-reply completion.
`RemoteRequest` is the shared domain operation; `RemoteReply` pairs its result with response-window ownership.
The private `Request` envelope couples each operation to its typed completion in the same frame queue.
Runtime-bearing definitions stay inside `netio`; the synchronous facade exposes only opaque ownership. Fields are private.

```rust
use std::{collections::VecDeque, io, sync::Arc};
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

enum RemoteRequest {
    Program(PreparedProgram),
    Refuse(AdmissionError),
    Web(WebRequest),
}

enum Request {
    Preflight { program: ProgramText, completion: PreflightCompletion },
    Execute { request: RemoteRequest, completion: Completion },
}

struct PreflightCompletion {
    reply: oneshot::Sender<Result<PreparedProgram, AdmissionError>>,
    bytes: OwnedSemaphorePermit,
    cancelled: CancellationToken,
}

struct Completion {
    reply: oneshot::Sender<RemoteReply>,
    bytes: OwnedSemaphorePermit,
    cancelled: CancellationToken,
}

struct RemoteReply {
    body: ReplyBody,
    window: Arc<ResponseWindow>,
}

struct ResponseWindow {
    bytes: OwnedSemaphorePermit,
}

struct IoPort {
    requests: mpsc::Sender<Request>,
    budget: Arc<Semaphore>,
    wake: Arc<FrameWake>,
    cancelled: CancellationToken,
}

struct FrameInbox {
    requests: mpsc::Receiver<Request>,
    pending: VecDeque<Pending>,
    wake: Arc<FrameWake>,
}

async fn preflight(
    port: &IoPort,
    program: ProgramText,
) -> io::Result<Result<PreparedProgram, AdmissionError>>;

async fn exchange(port: &IoPort, request: RemoteRequest, window: u32) -> io::Result<RemoteReply>;

async fn connection(socket: tokio::net::TcpStream, port: IoPort) -> io::Result<()>;
```

`IoPort` owns the sender, shared budget and wake, and the cancellation child for its connection or link/program.
Child ports share the sender, budget and wake while deriving the appropriate cancellation child.
Both async exchanges use `io::Error` for channel closure, cancellation and IO failure; admission refusal is the
separate typed result of `preflight`, not a transport error.
`exchange` uses `port.budget.clone().acquire_many_owned(window)` before publishing `Request::Execute`.
It moves the permit into the completion. On application completion it moves into the `ResponseWindow`
owned by `RemoteReply` through `Arc`.
`preflight` publishes `Request::Preflight` under a small control window, releases it on completion and returns
its own typed oneshot result. The prepared value carries separate retained-byte ownership, not an output stream
or response window. Neither caller needs to match an impossible reply variant.
This wrapper only ties allocation lifetimes to the single permit; it has no reservation, resizing or routing API.
The permit itself is never split, cloned or shrunk. No encoder starts before completion reaches IO.
A closed oneshot drops the reply and its window immediately because no encoding job exists yet.
No speculative execution precedes admission. Cancelling an admission future drops its queue slot and byte permit.

Tokio types remain private to `netio`. The synchronous facade uses the same domain values and typed completions
as the channel envelopes above, without translating to a second request/reply vocabulary:

```rust
impl FrameInbox {
    fn drain(&mut self, clock: &impl FrameClock, frame: &mut impl FrameHandler);
}

trait FrameHandler {
    fn preflight(&mut self, program: ProgramText, completion: PreflightCompletion);
    fn start(&mut self, request: RemoteRequest, completion: Completion) -> Option<Pending>;
    fn resume(&mut self, pending: Pending) -> Option<Pending>;
}

impl PreflightCompletion {
    fn is_cancelled(&self) -> bool;
    fn finish(self, result: Result<PreparedProgram, AdmissionError>)
        -> Result<(), Result<PreparedProgram, AdmissionError>>;
}

impl Completion {
    fn is_cancelled(&self) -> bool;
    fn finish(self, body: ReplyBody) -> Result<(), ReplyBody>;
}
```

Each completion owns its typed oneshot sender, cancellation child and byte permit. `Completion::finish` pairs
its body and window into `RemoteReply`; preflight instead sends its admission result and releases the control window.
`Pending` owns a `Completion` and the existing interpreter continuation. Neither can be copied.
Finishing consumes the completion exactly once. A closed receiver returns the body or admission result for
immediate drop. It is cancellation, not a reason to retry a setter.
No tokio dependency enters core, DSP, HAL or the interpreter. Adding tokio-util and the facade confinement tests
belongs to the implementation PR, with ADR 0004 dependency evidence and approval.

### Admission and frame drain

SCPI request owners first complete the frame preflight described below to obtain an owned prepared program
and source/plan admission. They then obtain a response window, await `Sender::reserve` and publish through
its mpsc permit. Typed HTTP operations use their bounded source/plan admission and the same execution path.
No owner holds a queue slot while waiting for bytes. Execution publication is the FIFO program-start point;
preflight publication only requests resource analysis and cannot execute commands.
The owner signals the existing frame wake only after publication. The queue owns the work; the wake only prompts a check.
Back-pressure stops that connection's reads. VXI-11 link actors publish already accepted programs in link order;
their connection tasks can still handle read/clear/destroy RPCs. HTTP keeps its admission deadline and 503 policy.
There is no try-send-and-forget path for application requests or replies.

At intake, drain the wake before inspecting the queue. Use `try_recv`, never `blocking_recv` or await.
Visit at most eight requests, preflights or continuation turns. Stop taking work at 0.5 ms, checked between turns
with an injected clock. Alternate queued requests with eligible pending continuations, rotating pending owners
round-robin; preserve the scheduling cursor across quota yields. Start queued execution requests in publication
order. Neither a stream of new requests nor pending continuations may starve the other class.
A continuation waiting on OPC retains its fence and earlier results without holding the interpreter or inbox.
Check a waiting fence at most once per paced frame unless completion or cancellation wakes it sooner;
leave it pending and visit another owner if it is still waiting.
Completion, cancellation or the next paced frame permits another check. Preflight and HTTP turns remain eligible
beside waiting programs, including a preflight for the same VXI-11 link. Do not spin or re-signal solely because
OPC remains pending. Re-signal when the quota leaves runnable work, including a completion at the quota boundary.
The existing pollable wake and paced host intake stay. Remote work never forces an extra render.

Only one interpreter turn runs at a time. Commands within a turn remain ordered; a yield saves the program's
cursor, earlier results, operation identity and waveform selection/format for its tail. Shared interpreter defaults,
error queue and ESR remain instrument-wide. Resuming uses the saved selection without overwriting newer shared
selection changes from another client. This preserves the waveform result across OPC while allowing another
client to execute. Do not retain exclusive interpreter ownership through an indefinite acquisition wait.
A program collects bounded inline values and immutable encoding plans in command order, rather than full encoded
large responses. Plans freeze each query's data before a later setter can change it.
At most 1024 query descriptors fit the 1024-command limit. Their allocated metadata plus inline values must
fit 256 KiB per program; larger values use bounded source-backed streams. Preflight includes this bound.
After interpretation, the reply owns its source selection and immutable data. Only then does streaming begin;
an encoder cannot fill its channel and prevent the program from reaching oneshot completion.
Transmission and encoding never borrow mutable frame state or keep another program from starting.
Preserve query settlement, operation identity and error handling in the current interpreter.

### Byte bounds without blocking another client

Do not replace `reserve_available` with an acquisition of all 64 MiB for every unknown reply.
Holding that permit for a stalled socket would delay every other client until its send timeout.
The byte budget covers allocated capacity and peak overlap, not just encoded length.

Use bounded streaming for large results. Reserve a response window of at most 1 MiB before dispatch.
That window covers headers, inline output, chunk metadata, encoder scratch and both the chunk being written
and the next chunk being prepared. Use a capacity-one chunk channel to the existing bounded CPU workers.
Chunks are at most 256 KiB. Metadata and inline values use at most 256 KiB; headers and encoder scratch
share the remaining 256 KiB. Count allocated capacities, including containers, within these sublimits.
The encoder must stop producing when the channel is full. Reserve channel space asynchronously before submitting a
chunk job. A CPU worker encodes one ready chunk and returns; it never waits for a stalled socket's channel.
Bound and fairly schedule these chunk jobs so a stalled producer cannot occupy the shared workers.
No fully assembled second reply is allowed. Adapt existing encoders; do not keep parallel encoder implementations.
The reply, writer, every queued chunk and every scheduled encoding job hold an `Arc` to the same window.
The writer retains its owner while awaiting chunks and across every partial write. A job obtains its owner
before scheduling and keeps it through scratch destruction; its output chunk carries an owner until consumed
or dropped. Thus writer and encoder can hold live buffers concurrently without splitting the permit.
After the last successful write, the connection closes the chunk receiver, drops queued chunks and joins its
last job before dropping its writer buffers and window owner. Error, timeout and cancellation take the same
cleanup path after cancelling the producer. A blocking job cannot be detached or considered stopped by aborting
its async handle: wait for its completion acknowledgement after it has freed scratch and unsent output.
The permit returns only when the last buffer holder drops its window owner. Hold the connection/link slot
through this cleanup, including successful encoder shutdown. The frame never waits for that join.

The replacement uses a 64 MiB response-window semaphore shared by instrument and HTTP connection tasks.
Eight raw, eight VXI-11 and eight HiSLIP sockets, eighteen HTTP sessions and one UDP actor need at most
43 MiB of windows. VXI-11 may also have one submitted program per link, at most sixteen, bringing the maximum
to 59 MiB. At most one preflight per link adds sixteen 4 KiB control windows, keeping the total below 60 MiB.
Raw/HiSLIP preflight uses its connection's allowance instead of an additional window. Queued link programs acquire
no execution window until their predecessor completes.
Keep a connection slot occupied until its queued/active completion has finished or closed and every cancelled
encoder has stopped; replacement connections cannot accumulate windows through abandoned requests or workers. Thus admitted stalled sockets leave capacity for other
admitted work within these limits. Connection and link limits prevent monopolisation. Reserve fixed small windows for
control messages too. Never let a bulk encoder request a second window while holding its first.
The 64 MiB SCPI ceiling limits logical response bytes across chunks, including separators and terminators.
HTTP exports retain their own logical size limits. Streaming does not lower a client's maximum reply size.

### Retained sources and VXI-11 plans

Encoding streams carry immutable source leases or owned snapshots. The frame captures selection, format and
acquisition identity once. Existing record/snapshot ownership must supply stable data before it sends a plan.
Never hand a CPU worker a borrow of `Ui`. Count the entire capacity pinned by a lease, not just its selected
slice, and count old/new allocation overlap for copies or reconfiguration. Sharing an allocation counts it once;
its charge lives with that allocation until its last holder releases it. Plan containers and inline bytes retained
after a VXI-11 submission window ends also belong to this allowance, not to an uncharged output queue.

Derive the global retained-source/plan semaphore capacity `R` from the target memory plan and the existing watchdog defaults.
Compute every term in bytes: `1 MiB = 1,048,576 bytes`; the specified `Config.mem_budget` default of
512 MB is 512,000,000 bytes (about 488.28 MiB).

- `A` is the admitted acquisition peak, including reconfiguration overlap; `T` is all non-remote retained data.
- `O` is base, maps, all bounded request/framing buffers, HTTP body admission, fixed remote/runtime overhead and
  explicit headroom. Include the 64 MiB response-window ceiling separately; no allocation appears in two terms.
- Set `R = min(Config.mem_budget - A - T, 768 MiB - A - T - O - 64 MiB)`, with checked subtraction.
  A negative remainder rejects that memory plan.

The first term enforces `Config.mem_budget`, which bounds acquisition and retained data only, as defined in
[Budget arithmetic](README.md#budget-arithmetic). It excludes `O` and response windows because they belong to
separate process-memory classes. The second term enforces the whole-process soft-watchdog envelope and therefore
subtracts those classes too. Taking the minimum satisfies both limits without counting any allocation twice.

This is a hard allocation-capacity bound, not an RSS estimate or an allowance to consume free host memory.
The implementation must supply and test every term before cutover. Reconfiguration includes live remote charges;
it cannot reduce `R` below live ownership or silently increase it. No separate per-transport source pools exist.
M5b owns the complete sweep. Keep the existing watchdog policy and its default 768 MiB soft threshold.
Admission must also respect a lower configured soft limit (`ui/src/system/memory.rs`).

Before the first setter or destructive query, preflight the whole program on the frame thread through the
same inbox and frame-owned parser/exporter bounds. `Request::Preflight` and its typed oneshot form a side-effect-free
admission exchange, not another interpreter on IO. Parse using the existing grammar; retain syntax/command errors
for normal execution without applying setters, capturing query data, consuming errors/ESR or registering OPC events.
Include all query snapshots that may coexist, copies, full pinned allocations, plan metadata and inline bytes.
Use published capability maxima when a preceding setter, another client's turn or OPC makes the eventual source
size unknown. Do not execute commands to estimate it. For example, a byte-format waveform needs up to four source
bytes per sample for `f32` data, even though its wire output needs one. RESP_MAX alone is no source bound.
The frame's synchronous admission facade uses `try_acquire_many_owned` for the conservative retained peak and
returns either refusal or a `PreparedProgram` owning the parsed program and opaque retained-byte permit.
Tokio ownership stays inside the facade. Preflight scratch is bounded in `O`; any parsed state kept after the turn
is charged to the prepared value under `R`. A preflight never waits for resources or for an older program's OPC.
Execution consumes that same prepared value, captures actual query data at its interpreter turn and cannot
reparse into a larger resource requirement. Move retained ownership with the program, plans and encoder jobs
until all charged allocations are gone. Do not shrink it or seek more capacity after side effects.
Programs with no retained allocation beyond their window need no retained-byte permit. Parsed state retained
between preflight and execution still requires admission under `R`.
A source without a stable, bounded lease blocks cutover; never fall back to an uncharged assembled reply.

If source or plan admission cannot fit, reject the entire program before side effects. Raw SCPI and HiSLIP
record the existing `-363,"Output capacity exhausted"` error; a query completes with its empty-result terminator
or protocol end marker, and a write sends no SCPI reply. For raw SCPI/HiSLIP only, publish
`RemoteRequest::Refuse(error)` through `exchange` under a small control window. This frame turn records the
admission error and completes without executing the refused program. `AdmissionError` retains whether the program
contained a query so the completion can distinguish an empty result from no reply. Successful
preflight and VXI-11 refusal leave error/status state untouched. HTTP rejects admission with 503 before dispatch.
VXI-11 returns device_write error 9 (out of resources) and size 0; it has accepted none of that write.
An ordinary wait for a response window or frame queue space is backpressure, not partial execution.
No accepted setter is retried to obtain output space.

Each VXI-11 link owns at most 16 program entries in total: provisional preflight, accepted but unsubmitted,
submitted/pending, completed and still unread, or held by a read/encoder. Each entry holds at most 1024 query
descriptors and 256 KiB of allocated metadata/inline values. These totals include the submitted entry; completing it does not free its
place for another write while its output is unread. A completed write-only entry releases its place immediately.
Queued program text remains at most MAX_RECORD per link, including provisional preflight text and the
submitted program's retained text.
With 16 links server-wide, the bounds are 256 entries, 262,144 descriptors and 64 MiB of plan/inline capacity,
all additionally subject to `R`.
Each entry's logical reply is at most RESP_MAX (64 MiB, including terminators), so a link's logical backlog is
at most `16 * RESP_MAX`. This is a stream-length bound, not an allocated aggregate or a response-window size.
It preserves device_read spanning program replies above 64 MiB without a second XDR payload buffer.

For device_write, the link actor first takes a provisional entry and text capacity, then publishes one preflight
under a 4 KiB control window and awaits its own oneshot while the older submitted program may remain pending.
Only a successful `PreparedProgram` result transfers retained-byte ownership to the entry and permits a successful
write acknowledgement. No execution publication precedes that acknowledgement. The actor submits the prepared
entry later in link order. This extra control request does not consume the link's single execution position.
Entry/text exhaustion returns error 9/size 0 without publication; retained-byte exhaustion returns it after the
nonblocking frame preflight. Neither path discards previously accepted entries. Failed or cancelled preflight
releases the provisional entry, text and any returned prepared ownership; clear/destroy/disconnect cancels its
child too, and a late result cannot acknowledge or attach to a fresh link generation. While awaiting preflight,
the actor must still service those cleanup messages rather than await the result inside its mailbox handler.
A preflight uses bounded actor storage and owned permits, not new credits or mutable reservation bookkeeping.
Accepted entries already own the resources needed for eventual plans: submit them in order when the predecessor
finishes, even if its output remains unread. They acquire a response window only for submission;
completed plans already carry retained admission when that window is released. No later transfer or allocation
may fail because an acknowledged write omitted its plan/source allowance. Completed entries acquire an encoding
window only when read. Frame queues, retained bytes and HTTP body admission remain separate from windows.

### Lifetimes, sockets and protocol state

Use one `CancellationToken` tree: application, service, connection, program/transfer and its encoder job.
Stopping one service cancels only its subtree. HiSLIP clear cancels the old program child and creates a new child;
it does not cancel the session or fresh input. Full disconnect and timeout cancel the connection child.
The frame checks cancellation before starting or resuming work. Already applied setters stay applied;
there is no rollback or replay. No-reply programs still complete their oneshot.
Read EOF follows the transport rules in the audit. Do not infer disconnect from a lack of writable progress.

Task owners retain join handles. Shutdown stops admission, cancels children, closes the frame receiver and
drops queued/pending completions. IO asynchronously joins connection and encoding tasks before freeing runtime
state. Frame intake polls shutdown completion; it never joins a thread or waits for a service acknowledgement.
The mDNS adapter withdraws registrations and awaits daemon shutdown on IO, within its existing bound.

Writes use `tokio::time::timeout_at`. Keep one fixed total deadline and an idle deadline renewed only by a
positive write. A partial write retains its cursor. The next write uses the earlier of the two deadlines.
The total deadline also covers waits for encoding chunks. Timeout or WriteZero closes the connection and
cancels its encoder. No custom `SendTimer` or frame-thread socket timer remains.

Each listener keeps accepting after transient errors. Interrupted or aborted accepts retry; resource exhaustion
uses a capped asynchronous backoff and records the error. Configuration/bind failure reports failed service
status. Cancellation ends the loop. An accept error must not silently stop an otherwise enabled service.
Run ACL checks and interface/file reads through bounded blocking work where needed, never on the frame or
current-thread executor. Policy changes reach each actor through `watch`; disallowed held peers are cancelled.
Do not await a refusal write or a slow ACL probe in the accept loop. Admit a bounded refusal/probe task or close.

Transport tasks retain framing and wire state. Raw SCPI has no additional session state. VXI-11 link actors
own link ids, accepted write queues and output plans. They acknowledge accepted writes without waiting for OPC,
then submit prepared programs in order to the one frame queue. Each submitted program has its own oneshot;
a link keeps that receiver while handling bounded read/clear/destroy requests. No RPC call is sent to the frame just to parse XDR.
Each TCP actor awaits its own RPC result; link messages carry that call's oneshot, never routed reply ids.

The link actor serializes writes, reads, clear and destroy in mailbox order. A read selects only the completed
unread prefix available at its turn, freezes its length and moves the selected sources into the transfer;
later completions do not extend a selected prefix. Keep the deferred display-read case when the prefix is empty,
the head program is waiting for pixels and the RPC specifies a nonzero timeout. Wait within that timeout, then
select the completed prefix or return 15. Other empty reads return 15 immediately, including while waiting for
OPC. Keep END/REQCNT; invalid links return 4. Partial reads retain the
unread suffix and its entry ownership. A read-held entry still counts against the limit until its transfer
and encoder release it; a link cannot gain extra entries by starting stalled reads on other connections.
Concurrent read transfers can share an entry's immutable ownership; they never duplicate its descriptors or
consume the same unread bytes. The entry is released only after its unread suffix and every transfer are gone.

Clear cancels the old link child, including accepted unpublished work, the submitted continuation, unread plans
and unsent read output. Bytes already written cannot be recalled and applied setters stay applied.
Retain the link's submission position and old entry/window owners until its preflight and submitted oneshots
have finished or closed and its encoders have stopped, then acknowledge clear and install a fresh child for
post-clear writes.
Cancellation wakes frame intake to drop the old continuation; clear itself is not queued behind that program.
A late old completion is dropped during cleanup and cannot attach to the fresh child.
Destroy/disconnect use the same cleanup and retain the server-wide link slot until it finishes. Clear/destroy
need no frame-queue turn behind a pending OPC. These bounded link actors are protocol state, not parked replies.
HiSLIP session state pairs two connection actors through bounded control messages. The async actor can clear a
pending sync program without waiting behind that program in the frame queue. Keep its supported subset unchanged.

The VXI-11 portmapper has TCP connection actors and one UDP actor. The UDP actor owns its socket, 1500-byte
buffer and source address for the current datagram. It checks the ACL, answers GETPORT from the published
core-port value and sends to that datagram's peer. It needs no frame request, credit id zero or reply router.
Cancellation, service switches and errors cannot redirect a stale datagram reply to a later peer.

mdns-sd keeps its daemon. One IO adapter consumes owned configuration through `watch` and publishes status.
Only enabled listeners are advertised. Switch-off withdraws registrations and closes daemon sockets.
Packet handling, probing and interface refresh stay in the crate.

### HTTP large bodies and device control

Parse and validate the HTTP head on IO. Apply route limits, origin and ACL checks before 100 Continue and
before allocating the body. Take a single-large-body semaphore permit for bodies over 1 MiB.
The body owner holds it through transfer to its worker; the worker keeps it while the large allocation is live.
Reject a concurrent large body with 503 rather than queueing its bytes. Keep eight small readers bounded.
Static content stays on IO. Application operations use the same frame mpsc and oneshot as SCPI.
The eight screenshot-waiter and two large-output limits use owned permits, not shared counters or reply slots.

For POST `/update`, move the body into the existing signed-upload worker. Only its bounded header, job handle
and status cross frame intake. Keep the two-stage verification and signed installer boundary. Cancellation
before verification releases body admission. Once installation is accepted, the update job follows its existing
lifecycle independently of an HTTP peer disconnect. The frame neither hashes images nor holds the upload buffer.
Waveform imports also retain their large-body admission through parsing and transfer to bounded application state.

Redo WLAN #219 after the coherent IO replacement is accepted. One device task owns its wpa_supplicant control socket and request
sequence. Use bounded commands, oneshot results, a watched latest link status, tokio timers and the same
cancellation tree. Preserve scan, join, forget, persistence and the current timeout/error results.
Keep key derivation, filesystem work and disk sync on bounded workers. No radio wait or control-socket call
enters the frame. Remove the sleeping WLAN worker only after equivalent boundary tests pass.
USB bulk remains on the synchronous generator worker. SNTP and update checks can later reuse the actor pattern;
they are not prerequisites for this replacement.

### Why the four bug classes disappear

| Former failure | Structural reason |
|---|---|
| Splitting `OwnedSemaphorePermit` to emulate a mutable byte budget. | Acquire the full bounded window with `acquire_many_owned` before dispatch. Move one indivisible permit through completion, writing and encoder cleanup. The streaming bound removes resizing. |
| Dropping a reply after `try_send` fails. | Each request owns a unique oneshot sender. Its one send needs no queue capacity. The only send failure is a gone receiver, handled as cancellation. |
| `notify_one` waking only one of several waiters. | mpsc, oneshot, semaphore and cancellation futures own their wait registrations. `watch` publishes latest configuration. The only custom wake has one frame consumer and no message ownership. No shared `Notify` protocol exists. |
| A reply router inside a `select!` losing progress or blocking acceptance. | There is no router. A connection awaits its own oneshot, then writes its own socket. Cancellation may end that exchange, but cannot discard another connection's reply. Listener select branches only accept, reap tasks or stop. |

These guarantees depend on keeping the stated ownership and allocation bounds. Recreating a router, mutable
reservation facade or unbounded encoder channel would violate the design, even if it used tokio internally.

## Deletions and size

Delete `netport::Port`, `Ports`, `Credit`, `Budget`, `Reservation`, routed reply ids and flush/parking logic.
Delete `netio/runtime.rs`'s sessions reply map and IO wake socket, `session::receive`, `SendTimer`,
the custom HTTP `ReplyHandle`/waker state and sender counters, and the three duplicated frame intake loops.
Replace the frame's service-start/drop acknowledgement waits with nonblocking lifecycle polling.
Keep the existing frame wake, parser, protocol codecs, mdns-sd adapter, CPU encoders and signed-update worker.
Move reusable owned application values into the small facade rather than copying them into each transport.
Remove the `netport::Port` re-export and migrate its transport callers. Preserve other users of
`system::intake::Port`.

Original planning estimates from the 2026-11-18 audit, excluding tests, comments and blank lines:

| Scope | Original baseline estimate | Replacement allowance |
|---|---:|---:|
| `netio/` including discovery | about 1,230 | 850-1,050 |
| `netport.rs` and `netport/` including codecs/assets | about 1,020 | 350-450 |
| `system/intake.rs`, SCPI intake and VXI-11/HiSLIP ports | about 470 | 200-300 |
| Total comparable scope | about 2,720 | 1,400-1,800 |

This estimates a reduction of 920-1,320 production lines. Allow another 300 lines for streaming/source-lease
integration outside those paths: the whole replacement still removes at least about 620 lines.
Refresh the baseline from the implementation PR's parent. Measure the complete implementation diff,
including moved protocol state and helpers outside netio, with `cargo xtask loc --compare <git-ref>`.
Report binary and dependency cost too. Rework a design that grows code
by recreating the old abstractions. Keep the deletion and its replacement in the same cutover.

## Implementation and acceptance

Issue #220 requires one coherent implementation PR for the replacement. Develop raw SCPI first within that PR,
then move the other transports onto the same core. Temporary coexistence is a development stage, not a separately
merged core or transport-cutover PR. Do not split this into primitive-emulation PRs.

1. Introduce the facade, one frame inbox, raw connection actor, semaphore windows, cancellation tree and bounded
   encoding path. Adapt the existing interpreter without changing grammar or state. Prove stable snapshot
   ownership for every raw exporter. Delete superseded raw ports, reply routing and parking as callers move.
2. Move VXI-11 links, TCP/UDP portmapper, HiSLIP sessions and HTTP onto that same core. Retain mdns-sd lifecycle
   integration, signed upload and screenshot workers. Prove retained-source/plan admission for all exporters.
3. In that same PR, delete all remaining superseded netport/intake plumbing and temporary coexistence, run the
   boundary acceptance suite and measure production LOC over the complete replacement diff. The final deletion
   and production-LOC check are acceptance conditions of this PR, not deferred follow-ups.

Keep the fixed client and frame guarantees throughout development. No parser rebuild, new protocol,
acquisition rewrite or USB change belongs in this PR.

Test what clients observe. Carry over the existing fixtures cited in the audit, replacing tests of parked fields
with tests of delivery, cancellation and no replay. Cover at least:

- Fragmented, coalesced and pipelined programs; shared waveform/error/ESR state; atomic waveform selection through
  an OPC continuation; setter/query settlement; empty and binary replies; cancelled SINGLE and no tail replay.
- A blocked reader beside a healthy client, including a logical 64 MiB response. Fill all connection slots and
  encoder channels. The healthy peer completes without releasing the blocked writer. Verify memory high-water
  bounds and source-admission refusal before any setter/destructive query. Hold an encoder at a completion
  barrier after the final write and after cancellation: the single window permit and connection slot must remain
  held until writer, queued chunks and worker scratch are gone. A full channel must schedule no waiting CPU job.
- FIFO execution publication, the eighth turn, time-budget rollover and wake races before/during/after input polling.
  Hold an armed `:SINGLE;*OPC?` pending and complete independent raw, VXI-11, HiSLIP and HTTP operations before
  releasing it. Give all eight HiSLIP pending clients turns, preserving `all_eight_pending_connections_get_an_intake_turn`.
  Prove round-robin progress under continuous arrivals and quota rollover, saved waveform results after another
  client's selection changes, and release of every pending slot on cancellation. Carry forward the raw waveform
  result and the existing raw continuation progress contract. Cancelling queued and pending work never
  reruns a setter or delivers an old reply to a new connection.
- Partial writes, idle timeout, a trickle that reaches the total timeout, half-close, service off/on, ACL change,
  application shutdown and injected transient accept failures followed by a successful connection.
- VXI-11 TCP and UDP GETPORT beside eight TCP peers; fragmented records, pending writes, reads spanning replies
  above 64 MiB, END/REQCNT/errors and immediate empty-read timeout. Fill all 16 server-wide links and per-link
  entry/source limits; refusal acknowledges zero bytes and preserves earlier accepted writes. Queue a source-heavy
  write behind an indefinite OPC: before preflight completes there is no successful acknowledgement or side effect;
  after acknowledgement, exhaust remaining retained capacity and prove that write still executes with its original
  allowance. Change source sizes/selection between preflight and execution to exercise conservative bounds. Cancel
  preflight during clear/disconnect and deliver a late result; no stale acknowledgement or permit leak is allowed.
  Prove successful preflight and VXI-11 refusal leave setters, query data, errors/ESR and OPC registrations
  untouched. Cover read/clear
  ordering, cancellation of submitted and unpublished entries, late completions after clear, and release only
  after read/encoder cleanup. Exercise both count and allocation-capacity limits without speed assertions.
- HiSLIP pairing, negotiated fragmentation, pending OPC, async clear, fresh commands after clear and opt-in
  persistence. Discovery advertises only enabled listeners and withdraws on off without stopping other services.
- HTTP route results, MCP pending/empty/error results, local versus HTTP Cmd privileges, read deadlines, overload,
  origin/ACL rejection before Continue, large-body limits and admission release. Update tests must prove the
  image bytes and full-image verification stay off frame, invalid signatures never install, and accepted
  updates keep their lifecycle. Cover deferred HTTP/MCP capture and VXI-11 display-read timeout and cancellation.

Use paused tokio time for IO deadlines and explicit injected time for frame quotas and counters.
Use completion events and fake stalled writers, not real sleeps or elapsed-speed assertions.
Run UI tests in debug and test-release, the fast gate and the coordinator's full integration gate.
Repeat the accepted M15a stalled-reader workload on a DHO924S under an owner-released unit lease before rollout.
Record build, setup, picture rate, frame p50/p99, other-client wait, enqueue p99, throughput, RSS, CPU and wakeups.
Require at least 26 fps and other-client wait no worse than the supplied 36 ms reference in that workload.
Also retain the [performance budgets](README.md#performance-budgets). Host fixtures do not certify unit timing.

WLAN #219 is redone on the accepted core as described above, not carried forward as emulated ports.
Protocol slimming follows transport acceptance as a separate issue under #220; its issue number is not assigned
in this brief. It removes redundant protocol code against these boundary fixtures. It must not change the
supported VXI-11, HiSLIP or MCP surface without an explicit specification decision.
