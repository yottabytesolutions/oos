"""JSON row shapes for the read-only perfmap collector."""
from typing import TypedDict

Diagnostic = dict[str, int | str]
Diagnostics = dict[str, list[Diagnostic]]
Counters = dict[str, int]
CounterDelta = dict[str, int | None]
DRAW_NAMES = ('decay', 'fast', 'merge', 'records', 'grade', 'render', 'view_render')


class ContextSwitches(TypedDict, total=False):
    voluntary_ctxt_switches: int
    nonvoluntary_ctxt_switches: int


class Process(ContextSwitches):
    pid: int
    comm: str
    minflt: int
    cpu_ticks: int
    starttime: int
    processor: int


class Capture(TypedDict):
    threads: dict[str, Process]
    missing: list[str]
    raw: dict[str, str]
    interrupts: Counters | None
    policy0: Counters | None
    policy4: Counters | None
    gpu_trans_stat: Counters | None
    gpu_stat: Process | None
    ui_stat: Process
    clock_ticks: int | None


class ProcessDelta(TypedDict):
    comm: str
    same_identity: bool
    processor_before: int
    processor_after: int
    counters: CounterDelta


class Unavailable(TypedDict):
    unavailable: str


class Delta(TypedDict):
    ui: ProcessDelta
    threads: dict[str, ProcessDelta | Unavailable]
    expected_thread_churn: int
    ui_same_identity: bool
    interrupts: CounterDelta
    policy0: CounterDelta
    policy4: CounterDelta
    gpu_trans_stat: CounterDelta
    gpu: ProcessDelta | None


class State(TypedDict):
    started_s: float
    finished_s: float
    raw: str
    diagnostics: Diagnostics


class Snapshot(TypedDict):
    state: State
    proc_started_s: float
    proc_finished_s: float
    proc: Capture


class Measurement(TypedDict):
    schema: int
    point: str
    requested_seconds: float
    recorded_at: str
    state_interval_s: float
    proc_interval_s: float
    before: Snapshot
    after: Snapshot
    delta: Delta
