# Menu model and legacy surface

Status: specification draft for coordinator disposition review and owner approval, task 0064. Source baseline:
`main` e846b2cf (2026-11-18), compared with draft e9f7c30b. M17b-read reconciles the
E1 area owners, all 963 requirement rows and every indexed expansion below. This is a code extraction, not hardware
verification. It implements the specification
scope of [ADR 0038](../adr/0038-menus-and-parser.md) and rows 1 and 2 of
[Specification gaps](../architecture/spec-gaps.md). The [overview](overview.md) uses these same readers and links.

## Reading the inventory

The current baseline has **48 MenuId variants and 457 Item variants** (`ui/src/ui.rs`),
versus 38 and 331 in e9f7c30b. All 457 variants already had bindings in this inventory;
the old 456 summary was a counting error, not a missing control. Every menu variant is
listed below. Every Item variant is accounted for, including internal picker contexts. `Channel(c)` and `ChMore(c)`
are templates instantiated once per existing channel. Repeated indexed families explicitly state their expansion
and order. A row is a requirement, including its expansion, with a unique `menus-<n>` id. Removed controls remain as explicit tombstones; no id is reused.

Each section gives its path, constructor citation and runtime order. Unless a recipe says otherwise, table order is
item order. A conditional row occupies its listed position when present. `Open:X` means `Item::Open(MenuId::X)`; submenu bindings written as
`open_menu(X); parent=current menu` describe the preserved navigation contract; the merged handler calls
`open_submenu(X)` and also saves the ancestor/focus stack (`ui/src/ui.rs`). It
opens the named menu and changes no setting, except the legacy Cursor entry described below. Start-menu group
names are headings. Source paths in evidence cells name the current owning files. E1 moved dispatches to
`ui/src/ui/menu/`; delegated area helpers remain in their cited files. Historical tombstones
are explicitly marked and do not imply a live Item binding.
The final **Delta vs e9f7c30b** column records extraction history: Kept (same contract, citations refreshed),
Changed (including moved rows and corrected extraction), Added, or Dropped (removed from that surface). This is
separate from the rebuild disposition, which answers keep/change/drop under H1/N1. New ids start at menus-1200.
Rows share the rendering/navigation evidence below; item citations identify the concrete binding and value domain.

`m` means legacy `Model`, `Ui` means component/session state, `sys` means scope system settings. Bindings name the
actual field or handler, not invented future command variants. In M14/M17, setting edits return the corresponding
area command; component actions return events to their owner. The change stage applies settings and normalizes
limits, as [settings ownership](../architecture/README.md#settings-and-commands) requires. A keep disposition retains
behaviour and visibility, never direct writes from a menu. No new domain default is chosen here: opening a menu
reads the current state; it does not reset it. Reset/recall defaults remain the existing model/store contract.

CH = CH1..CHn for the platform (`ui/src/model.rs`); nonexistent channels are skipped by source
stepping (`ui/src/model.rs`). Numeric bounds below are **menu-local** bounds. “No local clamp” does not override
normalization, board limits or command validation. `d` is the signed step, with knob acceleration where currently
applied. Domain tables at the end resolve shared constants and dynamic sets.

Current highlight notation (`ui/src/ui/active_choice.rs`, `ui/src/ui.rs`):

- **S/P:** a choice is segmented if all labels fit; otherwise a picker. Both compare the committed
  `active_choice::index` with the ordered option index. Labels, including shortened or decorated values,
  do not identify choices. The active option is filled with ACCENT and contrasting dark text.
- **V:** a numeric value between minus and plus. Committed stepped selectors such as Setup slot receive
  the selected-value fill too. Numeric entry buffers and values without an option identity stay unfilled.
  Collapsed pickers use the same selected-value surface; focus remains an independent outline.
- Actions have focus borders, with filled state cues for measurements, Run, armed Single, History and
  populated On reference slots. Momentary destinations and status text have no committed option.
  ACCENT is `rgb(0xf4,0xf4,0xf4)`. An open picker follows remote state edits without moving focus.

H1's committed-choice presentation is implemented in the baseline. Inline submenu summaries and N1's
remaining navigation proposals below are still dispositions, not changes authorized by M17b-read.

Comparison with e9f7c30b: **963 table rows = 394 kept, 141 changed, 418 added, 10 dropped**.
The item/surface inventory subset is **903 rows = 353 kept, 122 changed, 418 added, 10 dropped**;
the other 60 rows are shared presentation, order/domain and data-model requirements (41 kept, 19 changed).
These are row counts, not expanded controls or feature-disposition totals. The 38 old menu section ids remain;
ten new MenuId variants and fourteen auxiliary surface sections have new ids. Added rows include newly inventoried
custom controls as well as upstream additions. Tombstone rows are menus-221, menus-304, menus-368..371, menus-559, menus-1569 and
menus-1619..1620. The removed modal's section id menus-1618 also remains reserved.

## Shared presentation and navigation requirements

| ID | Requirement | Disposition | Baseline evidence | Delta vs e9f7c30b |
|---|---|---|---|---|
| menus-1 | **H1, active choice.** Whenever a menu is open, every option selector shows its committed active option with a filled selection cue and readable contrasting text. A collapsed picker shows its current option as the same selected-value cue; expanding it highlights that option even when the row or focus changes. Segmented choices, toggles, sources, modes, analysis settings, view arrangements and analyser view selectors all use this rule. Compare stable option identity, never formatted/truncated display strings. Numeric entry buffers and momentary actions are not committed options. | keep committed identity and selected-value cue | `ui/src/ui/active_choice.rs`; `ui/src/ui.rs` | Kept |
| menus-2 | Every submenu entry shows its current value inline, for example `View: Bode`, `Math: M2 FFT`, `Cursor: Track`, `Decode: D1 UART`, `Acquisition: Average`. A view entry is selected when that view is actually displayed, including Scope, Spectrum, Eye/Jitter, Power, Bode, Bandwidth, Magnetic, Inverter, Double pulse and XY. Settings navigation alone does not select a view. A parent with several settings shows a compact identifying summary; an inactive feature says Off. Selection and keyboard focus remain visually distinct: fill for active, outline for focus. | change H1 | `ui/src/ui.rs`; `ui/src/ui/viewmore.rs` | Changed |
| menus-3 | Touch Menu opens Main; the same root opener closes it. Top: acquisition text opens Acquire in live mode and Reference in record mode; Measure opens the catalogue (MeasAdd), Cursor/Display/Storage their roots; network boxes open Network and USB opens Storage. Cards and Compact have different lower-bar paths, inventoried below. Root replacement clears the submenu chain. System is in Main, SIM under Analysis, Load file in Reference; none is a top button at this HEAD. | keep | `ui/src/ui.rs` | Changed |
| menus-4 | Front-panel Measure opens the catalogue; Cursor/Trigger Menu/Horizontal Menu/Math/Ref/Analyse open Cursor/Trigger/Acquire/Math/Ref/Decode. CHn selects/enables and opens its menu; a second press there turns it off. A held CH key/card toggles AC/DC. LA/GI preserve capability/probe guards. Either Multi knob opens Main when no menu is open. Dedicated knobs keep source/record context while menus are open. | keep | `ui/src/ui.rs` | Changed |
| menus-5 | Generic menus: Multi 1 rotates focus in items() order with wrapping; Multi 2 steps focused settings, never actions. Either Multi push opens picker, activates action/choice, or advances past numeric value. Touch segments select exact identity; -/+ step; picker value/label opens options. Text has no action. MeasAdd items() is empty, so generic Multi cannot navigate its custom catalogue; Selected layout-only Set/Clear/page controls and other custom surfaces also need the N1 mapping below. | keep generic behaviour; change N1 for custom control parity | `ui/src/ui.rs` | Changed |
| menus-6 | Submenus push all ancestors, focus and scroll/page state. Back returns one level with that state; root has no Back. Picker selection restores its owner/focus; picker Back cancels temporary entry. Close dismisses the chain. Back and Close must also be focusable with either Multi push. The merged menu_history already restores ancestor menu/focus, but complete scroll restoration and knob exits remain N1 work. | change N1: complete return state and knob exits | `ui/src/ui.rs` | Changed |
| menus-7 | Plot taps outside a menu close it and consume the tap; another root opener may switch menus. Dialogs anchor to the relevant Cards/Compact control or top opener, clamped to screen. Catalogue, Selected and settings use their own layouts. Keep logical order, overflow scrolling/paging and visible Back/Close at all text sizes. Rebuilt touch targets are at least 40 px, including the smaller legacy popups, zoom and modal controls. | change N1: complete overflow/return paths; enlarge smaller legacy targets | `ui/src/ui.rs`; `ui/src/ui/workspace.rs` | Changed |
| menus-8 | Touch lock suppresses touch edits, drags and pinches; Menu unlocks without opening a menu. Physical keys/knobs remain available. Full screen is ineffective while locked so the unlock path remains visible. On-screen keyboard and dialogs take input first, retaining existing Force/Power handling. One-shot start actions keep their existing close behaviour; slope/history stepping keeps Main open. | keep | `ui/src/ui.rs` | Kept |
| menus-9 | Rebuild acceptance covers every inventory expansion and condition on host fakes: ordered rows, exact commands/events, boundaries, all text sizes, each active option with focus elsewhere, picker return, nested Back/Close by touch and knobs, touch lock, capability variants and remote changes while open. Golden images prove the selection/focus distinction for each control kind, submenu summaries and every view. Disabled/unavailable actions keep their documented rejection/no-op result until separately approved. | keep behaviour; add M17 acceptance evidence | `ui/src/ui/lint.rs`; `ui/src/ui.rs`; `ui/src/ui/viewmore.rs` | Kept |

H1 changes presentation only. It does not change a value set, silently turn on a source or start a view/sweep. N1 is
a navigation completion: the merged menu_history stack restores menu and focus, but not scroll positions or complete knob exits.
Main > System > Network > Remote > SSH now returns through its ancestors by touch. New overview entry points are specified in overview.md; they do not rename existing inventory rows.

## Ordered menu inventory

Every row below inherits H1/N1. Generic Item menus use the shared touch/Multi behaviours;
custom surfaces explicitly record their current touch-only gaps. Tombstones do not participate in runtime order. “Always” means visible and interactable whenever
that menu exists, with handler preconditions preserved. Legacy menus generally do not grey out invalid actions:
they show a toast or do nothing, as recorded in the binding/domain cells. Conditional visibility is not disablement.

### Main

Path: **Menu**. Menu requirement: **menus-10**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/navigation.rs`.

Group order: Acquisition = RunStop..Open:Acquire; Trigger = Open:Trigger..Open:Zone; Analysis = Measure, Cursor, Math, Ref, SIM, Decode, Bus, Mask; Analysers = Spectrum, Eye, Power, Bode, Bandwidth, Identify, Magnetic, Inverter, Double pulse; Instruments = Open:Afg..Open:Logger; Display = Open:Display..NavFwd; System = Open:Utility, Open:Storage, Shot, QuickRun, LockAct, PowerAct. Omit Afg without generator capability, La without LA port, Older/Newer without hist_pos. Features omits disabled analyser/SIM entries; Identify follows Bandwidth and disappears with bwtest.
The group headings are not selectable items.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-100 | `RunStop`: Run/Stop | action | `key::RUNSTOP`; toggle run; leaving stopped Single changes sweep to Auto | Always | filled while Run | keep | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-101 | `SingleAct`: Single | action | `key::SINGLE; change::single_key`; arm Single capture; close menu | Always | filled while armed Single + Run | keep | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-102 | `AutosetAct`: Autoset | action | `key::AUTO`; auto setup; close menu | Always | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-103 | `ClearDisp`: Clear | action | `clear_display()`; clear accumulated display; closes Main when invoked there | Always | focus border only | keep | `ui/src/ui/menu/display.rs` | Kept |
| menus-104 | `DefaultAct`: Default | action | `key(DEFAULT)`; reset setup and default measurement selection; retain Quick/text size; reset selected workspace CH, per-row measurement overrides/config, stats, XY and mask; close menu | Always | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Changed |
| menus-105 | `Open:Acquire`: Acquire | submenu | `open_menu(Acquire); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/acquire.rs` | Kept |
| menus-106 | `Open:Trigger`: Trigger | submenu | `open_menu(Trigger); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-107 | `SlopeAct`: Slope | action | `key::SLOPE; m.slope`; cycle Rising, Falling, Either; retain menu | Always | focus border only; slope value absent | change H1: inline active slope and selection cue | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-108 | `ForceAct`: Force | action | `key::FORCE; force_trigger`; force eligible acquisition; close menu | Always | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-109 | `Open:Zone`: Zones | submenu | `open_menu(Zone); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/zone.rs` | Kept |
| menus-110 | `Open:Measure`: Measure | submenu | `open_submenu(MeasAdd)` from Main; catalogue with current sources; not the Measure settings root | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/measure.rs` | Changed |
| menus-111 | `Open:Cursor`: Cursor | submenu | `open_menu(Cursor); parent=current menu`; navigate; current summary required by H1; Off becomes Time on entry | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/cursor.rs` | Kept |
| menus-112 | `Open:Math`: Math | submenu | `open_menu(Math); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/math.rs` | Kept |
| menus-113 | `Open:Ref`: Reference | submenu | `open_menu(Ref); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-1200 | `Open:Sim`: SIM | submenu | `open_submenu(Sim)`; navigation only; H1 inline current summary | sim feature offered | focus border; no active summary | change H1: inline summary and active-view cue | `ui/src/ui/menu/simulation.rs` | Added |
| menus-114 | `Open:Decode`: Decode | submenu | `open_menu(Decode); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/decode.rs` | Kept |
| menus-1201 | `Open:Bus`: Bus | submenu | `open_submenu(Bus)`; navigation only; H1 inline current summary | Always | focus border; no active summary | change H1: inline summary and active-view cue | `ui/src/ui/menu/bus.rs` | Added |
| menus-115 | `Open:Mask`: Mask | submenu | `open_menu(Mask); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/mask.rs` | Kept |
| menus-116 | `Open:Spectrum`: Spectrum | submenu | `open_menu(Spectrum); parent=current menu`; navigate; current summary required by H1 | Feature spectrum offered by System > Features | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysers.rs` | Changed |
| menus-117 | `Open:Eye`: Eye / jitter | submenu | `open_menu(Eye); parent=current menu`; navigate; current summary required by H1 | Feature eye offered by System > Features | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysers.rs` | Changed |
| menus-118 | `Open:Power`: Power analysis | submenu | `open_menu(Power); parent=current menu`; navigate; current summary required by H1 | Feature power offered by System > Features | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysers.rs` | Changed |
| menus-119 | `Open:Bode`: Bode | submenu | `open_menu(Bode); parent=current menu`; navigate; current summary required by H1 | Feature bode offered by System > Features | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/analysers.rs` | Changed |
| menus-120 | `Open:BwTest`: Bandwidth | submenu | `open_menu(BwTest); parent=current menu`; navigate; current summary required by H1 | Feature bwtest offered by System > Features | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/bandwidth.rs`; `ui/src/ui/analysers.rs` | Changed |
| menus-121 | `IdentifyAct`: Identify | action | `identify_now(); MenuId::Identify`; identify signals and open findings | Bandwidth feature offered; follows Bandwidth in its registry start entries | focus border only | keep | `ui/src/ui/menu/identify.rs`; `ui/src/ui/bwtest.rs` | Changed |
| menus-1202 | `Open:Magnetic`: Magnetic | submenu | `open_submenu(Magnetic)`; navigation only; H1 inline current summary | magnetic feature offered | focus border; no active summary | change H1: inline summary and active-view cue | `ui/src/ui/menu/magnetic.rs` | Added |
| menus-1203 | `Open:Inverter`: Inverter | submenu | `open_submenu(Inverter)`; navigation only; H1 inline current summary | inverter feature offered | focus border; no active summary | change H1: inline summary and active-view cue | `ui/src/ui/menu/inverter.rs` | Added |
| menus-1204 | `Open:Dpt`: Double pulse | submenu | `open_submenu(Dpt)`; navigation only; H1 inline current summary | dpt feature offered | focus border; no active summary | change H1: inline summary and active-view cue | `ui/src/ui/menu/dpt.rs` | Added |
| menus-122 | `Open:Afg`: Generator | submenu | `open_menu(Afg); parent=current menu`; navigate; current summary required by H1 | platform caps.has_gen() | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/afg.rs`; `ui/src/ui/menu/generator.rs` | Kept |
| menus-123 | `Open:La`: Logic analyser | submenu | `open_menu(La); parent=current menu`; navigate; current summary required by H1 | la::available(); entry rejects missing probe while LA off | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/la.rs`; `ui/src/ui/menu/logic.rs` | Kept |
| menus-124 | `Open:Dvm`: DVM / counter | submenu | `open_menu(Dvm); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/dvm.rs` | Kept |
| menus-125 | `Open:Logger`: Data logger | submenu | `open_menu(Logger); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/logger.rs` | Kept |
| menus-126 | `Open:Display`: Display | submenu | `open_menu(Display); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-127 | `Open:Horizontal`: Horizontal | submenu | `open_menu(Horizontal); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/horizontal.rs` | Kept |
| menus-128 | `NavHist`: History | action | `key::NAVIGATE; Ui.hist_pos`; toggle latest stored history / Live; retain menu | Always | filled while hist_pos exists | keep | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-129 | `NavBack`: Older | action | `history_step(-1)`; older history/segment; stop acquisition; retain menu | hist_pos is Some | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-130 | `NavFwd`: Newer | action | `history_step(1)`; newer history/segment; stop acquisition; retain menu | hist_pos is Some | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-135 | `Open:Utility`: System | submenu | `open_submenu(Utility)`; first entry of Main System group (before Storage); current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/system.rs` | Changed |
| menus-131 | `Open:Storage`: Storage | submenu | `open_menu(Storage); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/storage.rs` | Kept |
| menus-132 | `Shot`: Screenshot | action | `Ui.shot_request`; close menu and save screenshot after next frame | Always | focus border only | keep | `ui/src/ui/menu/system.rs` | Kept |
| menus-133 | `QuickRun`: Quick | action | `quick(); m.quick`; run configured Quick action; close menu | Always | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-134 | `LockAct`: Touch lock | action | `key::TOUCHLOCK; m.touch_lock`; toggle touch lock; close menu | Always | focus border only; lock state absent | change H1: inline current touch state and selection cue | `ui/src/ui/menu/navigation.rs` | Kept |
| menus-136 | `PowerAct`: Power off | action | `Ui.power_dlg`; close menu and open Power off/Restart/Cancel confirmation | Always | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |

### Channel(c)

Path: **CHn card / CHn key > CHn**. Menu requirement: **menus-11**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/channel.rs`.

Template instantiated for each existing CH1..CHn, c=0..n_ch()-1, in channel order.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-137 | `ChOn`: Display | toggle (segmented/picker) | `m.ch[c].on`; Off, On; c must exist; enabling during Run floors live time/div | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/channel.rs` | Changed |
| menus-138 | `ChScale`: Scale | numeric | `m.ch[c].vscale/vfine; volts_per_div`; D-VScale; probe-scaled units/div; fine calibration rows; ground expansion rescales offset | Always | V | keep | `ui/src/ui/menu/channel.rs` | Kept |
| menus-139 | `ChOffset`: Offset | numeric | `m.ch[c].offset`; channel unit; step V/div / 25; no local clamp | Always | V | keep | `ui/src/ui/menu/channel.rs` | Kept |
| menus-140 | `ChCoupling`: Coupling | segmented choice / picker | `m.ch[c].coupling`; board().couplings(); D-Coupling | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/channel.rs` | Kept |
| menus-141 | `ChBw`: Bandwidth | segmented choice / picker | `m.ch[c].bw`; D-BW; ordered 31 filter choices | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/channel.rs` | Kept |
| menus-142 | `ChProbe`: Probe | segmented choice / picker | `m.ch[c].probe`; 1x, 10x, 100x | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/channel.rs` | Kept |
| menus-143 | `ChInvert`: Invert | toggle (segmented/picker) | `m.ch[c].invert`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/channel.rs` | Kept |
| menus-144 | `ChFine`: Fine scale | toggle (segmented/picker) | `m.fine_v`; Off, On; shared across channels | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/channel.rs` | Kept |
| menus-145 | `ChLabel`: Name | action | `m.ch_label[c]; Osk::for_label`; channel name keyboard | Always | focus border only | keep | `ui/src/ui/menu/channel.rs` | Kept |
| menus-146 | `Open:ChMore`: More (same channel) | submenu | `open_menu(ChMore); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/channel.rs` | Kept |

### ChMore(c)

Path: **CHn > More**. Menu requirement: **menus-12**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/channel.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-147 | `ChUnit`: Unit | segmented choice / picker | `m.ch[c].unit`; V, A, W, U | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/channel.rs` | Kept |
| menus-148 | `ChBias`: Zero bias | numeric | `m.ch[c].bias`; channel unit; step 0.01 V/div; no local clamp | Always | V | keep | `ui/src/ui/menu/channel.rs` | Kept |
| menus-149 | `ChBiasZero`: Zero this input | action | `zero_input(c)`; zero selected input from its measurement | Always | focus border only | keep | `ui/src/ui/menu/channel.rs` | Kept |
| menus-150 | `ChSkew`: Deskew | numeric | `m.ch[c].skew`; s; step 600 ps; clamp -100..100 ns | Always | V | keep | `ui/src/ui/menu/channel.rs` | Kept |
| menus-1205 | `AutoDeskew`: Auto deskew | action | `auto_deskew()`; align shown channel/reference edges in deskew_ch/deskew_ref; no analogue skew calibration writes | Always | focus border only | keep | `ui/src/ui/menu/horizontal.rs` | Added |
| menus-151 | `ChExpand`: Expand around | toggle (segmented/picker) | `m.v_expand_gnd`; Center, Ground | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/channel.rs` | Kept |

### Horizontal

Path: **Menu > Display group > Horizontal; Acquire > Horizontal; TIME card**. Menu requirement: **menus-13**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/horizontal.rs`.

ZScale and ZDelay occur only while zoom is on. New AutoDeskew/ClearDeskew follow HZero before Zoom;
all other rows retain this order.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-152 | `HScale`: Time/div | numeric | `m.tscale/tfine; time_per_div`; T-scale, s/div; fine 1% rounded to 3 digits before roll; preserves expansion point; merged T-scale now begins at 50 ps, live ADC floor 1 ns | Always | V | keep | `ui/src/ui/menu/horizontal.rs` | Changed |
| menus-153 | `HFine`: Fine time base | toggle (segmented/picker) | `m.fine_t`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/horizontal.rs` | Kept |
| menus-154 | `HExpandI`: Expand around | segmented choice / picker | `m.h_expand`; Center, Left, Right, Trigger | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/horizontal.rs` | Kept |
| menus-155 | `HDelay`: Delay | numeric | `m.delay`; s; step time/div / 25; no local clamp | Always | V | keep | `ui/src/ui/menu/horizontal.rs` | Kept |
| menus-156 | `HZero`: Delay to 0 | action | `m.delay`; set 0 s | Always | focus border only | keep | `ui/src/ui/menu/horizontal.rs` | Kept |
| menus-1206 | `AutoDeskew`: Auto deskew | action | `auto_deskew()`; align shown traces by their edges | Always | focus border only | keep | `ui/src/ui/menu/horizontal.rs` | Added |
| menus-1207 | `ClearDeskew`: Clear deskew | action | `clear_deskew()`; zero software deskew for channels/references | Always | focus border only | keep | `ui/src/ui/menu/horizontal.rs` | Added |
| menus-157 | `Zoom`: Zoom | toggle (segmented/picker) | `m.zoom; set_zoom`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/horizontal.rs` | Kept |
| menus-158 | `ZScale`: Zoom time/div | numeric | `m.zscale`; T-scale, s/div; clamped step; merged T-scale now begins at 50 ps, live ADC floor 1 ns | zoom on | V | keep | `ui/src/ui/menu/horizontal.rs` | Changed |
| menus-159 | `ZDelay`: Zoom position | numeric | `m.zdelay`; s; step zoom s/div / 25; no local clamp | zoom on | V | keep | `ui/src/ui/menu/horizontal.rs` | Kept |
| menus-160 | `AutoRoll`: Auto Roll | toggle (segmented/picker) | `m.auto_roll`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/horizontal.rs` | Kept |

### Trigger

Path: **Menu > Trigger group > Trigger; Trigger key/card**. Menu requirement: **menus-14**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/trigger.rs`.

The table is the union of rows. Runtime order is the trigger recipe below, not union-table order.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-161 | `TrigType`: Type | segmented choice / picker | `m.trig_type; set_trig_type`; TRIG_TYPES unchanged. Hardware preserves shared channels/source/level/slope, gives second source a valid on channel; demo applies presets. Reference triggers fit levels when invalid/type requires it and reset replay search. | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs`; `ui/src/ui/trigger_controls.rs` | Changed |
| menus-162 | `TrigSrc`: Source | segmented choice / picker | `m.trigger_wave(); set_trigger_source`; existing CH followed by loaded R1..R10; LA source choices on eligible LA trigger path. Primary source can switch live/record owner, preserving Run/Stop intent; secondary only resets search. | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs`; `ui/src/ui/trigger_controls.rs` | Changed |
| menus-163 | `TrigSlope`: Slope | segmented choice / picker | `m.slope`; Rising, Falling, Either | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-164 | `TrigLevel`: Level | numeric | `m.trig_level`; source units; step source V/div / 25; reference uses trigger_vpd and resets replay search; no local clamp | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs`; `ui/src/ui/trigger_controls.rs` | Changed |
| menus-165 | `Trig50`: Level 50 % | action | `level_50()`; set waveform midpoint if source statistics valid | trigger recipe below | focus border only | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-166 | `TrigCoup`: Coupling | segmented choice / picker | `m.trig_coup`; DC, AC, LF reject, HF reject | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-167 | `DecProto`: Protocol | segmented choice / picker | `m.dec_proto; set_dec_proto`; enabled D-Protocols registry. Existing reference-source context sets protocol and only applies la_off threshold/input override. Otherwise hardware preserves shared channels/source/trigger and allocates new roles from free on channels; demo applies module setup and enables needed sources. Newly changed non-reference protocol resets own options and may open Bus view pane. | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs` | Changed |
| menus-168 | `TrigBusWhat`: Match | segmented choice / picker | `m.trig_bus`; Frame, ID/Addr, Data, Error | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-169 | `TrigBusVal`: Value | picker (numeric hex entry) | `m.trig_bus_val; set_hex`; 0..0xff_ffff_ffff; hexadecimal keypad; step 1 | trigger recipe below | V; numeric keypad keys have focus only | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-170 | `SerRel`: Relation | segmented choice / picker | `m.ser.addr_rel (I2C ID) / m.ser.rel`; =, ≠, >, <, In range, Out of range (RELS) | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/serial.rs`; `ui/src/ui/sertrig.rs` | Kept |
| menus-171 | `SerMax`: Max | picker (numeric hex entry) | `m.ser.addr_max (I2C ID) / m.ser.max; set_hex`; 0..0xff_ffff_ffff entry; I2C addr_max clamps to 0..0x3ff (u32) | trigger recipe below | V; numeric keypad keys have focus only | keep | `ui/src/ui/sertrig.rs`; `ui/src/ui/menu/serial.rs` | Kept |
| menus-172 | `SerMask`: Mask | picker (numeric hex entry) | `m.ser.mask; set_hex`; 0..0xff_ffff_ffff; step 1; all-ones entry stores u64::MAX | trigger recipe below | V; numeric keypad keys have focus only | keep | `ui/src/ui/sertrig.rs`; `ui/src/ui/menu/serial.rs` | Kept |
| menus-173 | `UartErr`: Error type | segmented choice / picker | `m.ser.uart_err`; Any, Parity, Framing | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/serial.rs`; `ui/src/ui/sertrig.rs` | Kept |
| menus-174 | `UartPol`: Polarity | toggle (segmented/picker) | `m.ser.uart_inv`; Normal, Inverted | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/serial.rs`; `ui/src/ui/sertrig.rs` | Kept |
| menus-175 | `UartStop`: Stop bits | segmented choice / picker | `m.ser.uart_stop`; 1, 1.5, 2 stop bits | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/serial.rs`; `ui/src/ui/sertrig.rs` | Kept |
| menus-176 | `SpiCs`: CS | toggle (segmented/picker) | `m.ser.spi_cs_high`; Active low, Active high | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/serial.rs`; `ui/src/ui/sertrig.rs` | Kept |
| menus-177 | `SpiTimeout`: Timeout | numeric | `m.ser.spi_timeout`; Off then 1-2-5 ladder 100 ns..10 ms; clamped | trigger recipe below | V | keep | `ui/src/ui/sertrig.rs`; `ui/src/ui/menu/serial.rs` | Kept |
| menus-178 | `I2cAddrBits`: Address | toggle (segmented/picker) | `m.ser.ten_bit`; 7-bit, 10-bit | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/serial.rs`; `ui/src/ui/sertrig.rs` | Kept |
| menus-179 | `I2cBytes`: Bytes | segmented choice / picker | `m.ser.bytes`; Any (0), 1, 2, 3, 4, 5 | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/serial.rs`; `ui/src/ui/sertrig.rs` | Kept |
| menus-180 | `I2cDir`: Direction | segmented choice / picker | `m.ser.dir`; Either, Write, Read | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/serial.rs`; `ui/src/ui/sertrig.rs` | Kept |
| menus-181 | `TrigPol`: Polarity | segmented choice / picker | `m.trig_pol`; Positive, Negative | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-182 | `TrigWhen`: When | segmented choice / picker | `m.trig_when`; >, <, Inside, Outside; prepend Off only for Runt | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-183 | `TrigT1`: Time | numeric | `m.trig_t1`; T-trigger, s; clamped step | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-184 | `TrigT2`: Upper limit | numeric | `m.trig_t2`; T-trigger, s; clamped step | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-185 | `TrigLevel2`: Level 2 | numeric | `m.trig_level2`; source units; step source V/div / 25; reference uses trigger_vpd and resets replay search; no local clamp | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs`; `ui/src/ui/trigger_controls.rs` | Changed |
| menus-186 | `VidStd`: Standard | segmented choice / picker | `m.video_std`; PAL, NTSC | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-187 | `VidSync`: Sync | segmented choice / picker | `m.video_sync`; Lines, Line n, Odd, Even, Fields | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-188 | `VidLine`: Line | numeric | `m.video_line`; 1..video_std.lines()/2 - 10; step 1 | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-189 | `Pat:0`: CH1 | segmented choice / picker | `m.pat[0]`; Pattern H, L, X, Rise, Fall; Duration H, L, X | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-190 | `Pat:1`: CH2 | segmented choice / picker | `m.pat[1]`; Pattern H, L, X, Rise, Fall; Duration H, L, X | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-191 | `Pat:2`: CH3 | segmented choice / picker | `m.pat[2]`; Pattern H, L, X, Rise, Fall; Duration H, L, X | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-192 | `Pat:3`: CH4 | segmented choice / picker | `m.pat[3]`; Pattern H, L, X, Rise, Fall; Duration H, L, X | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-193 | `PatLevel`: Level | numeric | `m.pat_levels[m.trig_src]`; V; step source V/div / 25 | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-194 | `Open:LaPat`: D0-D15 | submenu | `open_menu(LaPat); parent=current menu`; navigate; current summary required by H1 | Pattern/Duration and la_on() | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/la.rs`; `ui/src/ui/menu/logic.rs` | Kept |
| menus-195 | `WinModeI`: Trigger on | segmented choice / picker | `m.win_mode`; Enter, Exit, Inside | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-196 | `TrigSrc2`: Source B | segmented choice / picker | `m.trigger_wave2(); set_trigger_source`; existing CH followed by loaded R1..R10; LA source choices on eligible LA trigger path. Primary source can switch live/record owner, preserving Run/Stop intent; secondary only resets search. | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs`; `ui/src/ui/trigger_controls.rs` | Changed |
| menus-197 | `TrigSlope2`: Slope B | segmented choice / picker | `m.trig_slope2`; Rising, Falling, Either | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-198 | `Setup`: Setup | numeric | `m.setup`; T-trigger, s; clamped step | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-199 | `Hold`: Hold | numeric | `m.hold`; T-trigger, s; clamped step | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-200 | `NthIdle`: Idle time | numeric | `m.nth_idle`; T-trigger, s; clamped step | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-201 | `NthN`: Edge no. | numeric | `m.nth_n`; 1..65535; step 1 | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-202 | `TrigNoise`: Noise reject | toggle (segmented/picker) | `m.trig_nrej`; Off, On | trigger recipe below; not Protocol and no LA trigger line | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-203 | `TrigAutoNoise`: Auto noise rej. | toggle (segmented/picker) | `m.auto_nrej`; Off, On | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-204 | `TrigMode`: Sweep | segmented choice / picker | `m.trig_mode; m.run`; Auto, Normal, Single; choosing Single arms Run | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-205 | `SearchOn`: Mark events | toggle (segmented/picker) | `Ui.search_on`; Off, On | trigger recipe below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-206 | `SearchTable`: Event list | action | `table_toggle(Search)`; open/close trigger event list | trigger recipe below; search_on | focus border only | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-207 | `SearchSave`: Save events | action | `events_export(Search)`; export trigger events | trigger recipe below; search_on | focus border only | keep | `ui/src/ui/menu/trigger.rs` | Kept |
| menus-208 | `Open:Zone`: Zones | submenu | `open_menu(Zone); parent=current menu`; navigate; current summary required by H1 | trigger recipe below | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/zone.rs` | Kept |
| menus-209 | `TrigHoldoff`: Holdoff | numeric | `m.holdoff`; Off (0), 1-2-5 ladder 100 ns..10 s | trigger recipe below | V | keep | `ui/src/ui/menu/trigger.rs` | Kept |

### Acquire

Path: **Menu > Acquisition group > Acquire; Horizontal Menu key; acquisition top-bar readout**. Menu requirement: **menus-15**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/acquire.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-210 | `AcqMode`: Mode | segmented choice / picker | `m.acq`; Normal, Peak, Average, High res | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/acquire.rs` | Kept |
| menus-211 | `AcqAvg`: Averages | numeric + picker | `m.avg_count`; 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024 | Average only | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/acquire.rs` | Kept |
| menus-212 | `AcqAvgHw`: HW average | toggle (segmented/picker) | `m.avg_hw`; Off, On; warns/falls back if board().avg_hw rejects setup | Average and hardware source | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/acquire.rs` | Kept |
| menus-213 | `AcqDepth`: Memory | segmented choice / picker | `m.mdepth; depth_label()`; D-Depth; choice and effective channel-capped depth both shown | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/acquire.rs` | Kept |
| menus-214 | `AcqSeg`: Segments | segmented choice / picker | `m.segments; seg_count()`; Off, 10, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000, 50000, filtered to seg_limit plus limit itself | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/acquire.rs` | Kept |
| menus-215 | `AcqSegHw`: HW segments | toggle (segmented/picker) | `m.seg_hw`; Off, On; reclamps segment count | segments > 0 | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/acquire.rs` | Kept |
| menus-216 | `AcqSegIvl`: Seg. interval | segmented choice / picker | `m.seg_ivl`; None (0), 1 us, 10 us, 100 us, 1 ms, 10 ms, 100 ms, 1 s | seg_hw_on(): hardware source + seg_hw + positive count + supported board setup | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/acquire.rs` | Kept |
| menus-217 | `AcqSegTable`: Segment times | action | `table_toggle(Segments)`; open/close segment timestamp table | seg_hw_on() | focus border only | keep | `ui/src/ui/menu/acquire.rs` | Kept |
| menus-1208 | `AcqCores`: ADC cores | segmented choice / picker | `m.cores`; Raw, Matched; software core offset matching | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/acquire.rs` | Added |
| menus-1209 | `AcqDrawPath`: Draw path | segmented choice / picker | `m.draw_batch`; Standard, Batch; accepts the edit through change detection | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/acquire.rs` | Added |
| menus-1210 | `AcqAlias`: Aliasing | status | `alias_above()`; below max sample rate except Peak: above sample-rate/2 folds down; read-only warning | alias_above().is_some() | text only | keep | `ui/src/ui/menu/acquire.rs` | Added |
| menus-218 | `Open:Horizontal`: Horizontal | submenu | `open_menu(Horizontal); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/horizontal.rs` | Kept |

### Measure

Path: **Menu > Analysis > Measure; Measure key/top bar**. Menu requirement: **menus-16**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/measure.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-219 | `MeasSrc`: Source A | segmented choice / picker | `Ui.meas_src`; W-source domain (existing channels then loaded reference slots); step_wave cycles | Settings and catalogue source header; A/B also in Pairs header | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs`; `ui/src/ui/workspace.rs` | Changed |
| menus-220 | `MeasSrcB`: Source B | segmented choice / picker | `Ui.meas_src_b`; W-source domain (existing channels then loaded reference slots); step_wave cycles | Settings and catalogue source header; A/B also in Pairs header | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs`; `ui/src/ui/workspace.rs` | Changed |
| menus-221 | `Open:MeasAdd`: Add item | submenu | `open_menu(MeasAdd); parent=current menu`; navigate; current summary required by H1 | Removed from this surface: Measure settings no longer has an Add item row; Measure opens the catalogue directly | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | e9f7c30b historical: `ui/src/ui.rs`; `ui/src/ui.rs`; current constructor `ui/src/ui.rs` | Dropped |
| menus-1211 | `MeasCopyTo`: Copy meas. to | destination choice (S/P), momentary copy | `copy_measurements(meas_src, picked wave)`; W-source excluding A; copies singles/pairs/catalogue, skips duplicates, refuses capacity overflow | Always | value Pick; no option selected because no committed destination | keep: destination triggers a copy, not a retained option; no H1 selection implied | `ui/src/ui/menu/measure.rs` | Added |
| menus-1212 | `MeasGate`: Gate | segmented choice / picker | `m.meas_full_record; set_measurement_full_record`; Screen, Full record | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs`; `ui/src/ui/tools.rs` | Added |
| menus-1213 | `MeasRefs`: Refs | segmented choice / picker | `m.meas_lpct/hpct`; 10/90, 20/80 percent; reset/recompute measurement references | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs`; `ui/src/ui/tools.rs` | Added |
| menus-224 | `MeasStats`: Statistics | toggle (segmented/picker) | `Ui.stats_on`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs` | Kept |
| menus-225 | `MeasLabels`: Labels on trace | toggle (segmented/picker) | `m.meas_labels`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs` | Kept |
| menus-1214 | `StripDock`: Dock | segmented choice / picker | `m.strip_dock`; Under, Left, Right, Above; clamps steps; docks statistics too | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs` | Added |
| menus-1215 | `MeasIdle`: Idle | numeric | `m.meas_idle`; period (0) then 1-2-5 from 1 ns through 5 s; clamped | Always | V | keep | `ui/src/ui/menu/measure.rs`; `ui/src/ui/tools.rs` | Added |
| menus-1216 | `MeasTsetup`: Tsetup | numeric | `m.meas_tsetup`; 0 then 1-2-5 from 1 ns through 5 s; clamped | Always | V | keep | `ui/src/ui/menu/measure.rs`; `ui/src/ui/tools.rs` | Added |
| menus-1217 | `MeasThold`: Thold | numeric | `m.meas_thold`; 0 then 1-2-5 from 1 ns through 5 s; clamped | Always | V | keep | `ui/src/ui/menu/measure.rs`; `ui/src/ui/tools.rs` | Added |
| menus-1218 | `MeasOutside`: Outside | segmented choice / picker | `m.meas_out`; Above, Below, Both | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs`; `ui/src/ui/tools.rs` | Added |
| menus-1219 | `MeasN`: N periods | numeric | `m.meas_n`; 1..32, step 1 | Always | V | keep | `ui/src/ui/menu/measure.rs`; `ui/src/ui/tools.rs` | Added |
| menus-1220 | `MeasEdges`: Edges | segmented choice / picker | `m.meas_data`; Clock, Data | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs`; `ui/src/ui/tools.rs` | Added |
| menus-222 | `Open:MeasPair`: Pairs | submenu | `open_menu(MeasPair); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/measure.rs` | Changed |
| menus-223 | `Open:Dvm`: DVM / counter | submenu | `open_menu(Dvm); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/dvm.rs` | Kept |
| menus-226 | `MeasClear`: Clear all | action | `m.meas/meas2/cat; meas_own; meas_cfg; stats_reset`; clear standard, pair and module selections, overrides/config and statistics | Always | focus border only | keep | `ui/src/ui/menu/measure.rs` | Changed |

### MeasAdd

Path: **top Measure / Measure key / Main > Analysis > Measure**. Menu requirement: **menus-17**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/measure.rs`.

MeasAdd is now a custom catalogue, not items(MeasAdd) (which is empty). Categories and group headings below
determine runtime order; within each group use registered module order then Def declaration order. A Def may
appear in several categories with the same requirement id and same membership. This table retains the original
35 standard identities; the additional 25 standards, 9 pairs and 83 module results follow below. Settings opens
Measure; Selected opens MeasSelected. Source pickers and Gate are custom header controls. Tiles toggle membership,
fill while added, and separately outline the last selected tile. Catalogue tabs/groups and tiles are touch-only
today: the generic Multi path has no items to focus; N1 must provide equivalent key/knob access.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-227 | `MeasAdd:Vmax`: Maximum | toggle (button) | `m.meas; (Vmax, meas_src)`; catalogue id 2; toggle standard result on W-source A; unit V | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-228 | `MeasAdd:Vmin`: Minimum | toggle (button) | `m.meas; (Vmin, meas_src)`; catalogue id 3; toggle standard result on W-source A; unit V | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-229 | `MeasAdd:Vpp`: Peak-to-Peak | toggle (button) | `m.meas; (Vpp, meas_src)`; catalogue id 4; toggle standard result on W-source A; unit V | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-230 | `MeasAdd:Vtop`: Top | toggle (button) | `m.meas; (Vtop, meas_src)`; catalogue id 10; toggle standard result on W-source A; unit V | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-231 | `MeasAdd:Vbase`: Base | toggle (button) | `m.meas; (Vbase, meas_src)`; catalogue id 11; toggle standard result on W-source A; unit V | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-232 | `MeasAdd:Vamp`: Amplitude | toggle (button) | `m.meas; (Vamp, meas_src)`; catalogue id 1; toggle standard result on W-source A; unit V | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-233 | `MeasAdd:Vupper`: Upper Level | toggle (button) | `m.meas; (Vupper, meas_src)`; catalogue id 200; toggle standard result on W-source A; unit V | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-234 | `MeasAdd:Vmid`: Middle Level | toggle (button) | `m.meas; (Vmid, meas_src)`; catalogue id 201; toggle standard result on W-source A; unit V | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-235 | `MeasAdd:Vlower`: Lower Level | toggle (button) | `m.meas; (Vlower, meas_src)`; catalogue id 202; toggle standard result on W-source A; unit V | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-236 | `MeasAdd:Vavg`: Mean | toggle (button) | `m.meas; (Vavg, meas_src)`; catalogue id 7; toggle standard result on W-source A; unit V | Catalogue BASIC > AVERAGE & STATISTICS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-237 | `MeasAdd:Vrms`: RMS | toggle (button) | `m.meas; (Vrms, meas_src)`; catalogue id 8; toggle standard result on W-source A; unit V | Catalogue BASIC > AVERAGE & STATISTICS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-238 | `MeasAdd:VavgCyc`: Cycle Mean | toggle (button) | `m.meas; (VavgCyc, meas_src)`; catalogue id 203; toggle standard result on W-source A; unit V | Catalogue BASIC > PER CYCLE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-239 | `MeasAdd:VrmsCyc`: Cycle RMS | toggle (button) | `m.meas; (VrmsCyc, meas_src)`; catalogue id 204; toggle standard result on W-source A; unit V | Catalogue BASIC > PER CYCLE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-240 | `MeasAdd:VrmsAc`: AC RMS | toggle (button) | `m.meas; (VrmsAc, meas_src)`; catalogue id 9; toggle standard result on W-source A; unit V | Catalogue BASIC > AVERAGE & STATISTICS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-241 | `MeasAdd:StdDev`: Standard Deviation | toggle (button) | `m.meas; (StdDev, meas_src)`; catalogue id 206; toggle standard result on W-source A; unit V | Catalogue BASIC > AVERAGE & STATISTICS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-242 | `MeasAdd:Overshoot`: Positive Overshoot | toggle (button) | `m.meas; (Overshoot, meas_src)`; catalogue id 5; toggle standard result on W-source A; unit % | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-243 | `MeasAdd:Preshoot`: Negative Overshoot | toggle (button) | `m.meas; (Preshoot, meas_src)`; catalogue id 6; toggle standard result on W-source A; unit % | Catalogue BASIC > AMPLITUDE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-244 | `MeasAdd:Area`: Area | toggle (button) | `m.meas; (Area, meas_src)`; catalogue id 12; toggle standard result on W-source A; unit Vs | Catalogue BASIC > AVERAGE & STATISTICS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-245 | `MeasAdd:AreaCyc`: Cycle Area | toggle (button) | `m.meas; (AreaCyc, meas_src)`; catalogue id 205; toggle standard result on W-source A; unit Vs | Catalogue BASIC > PER CYCLE; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-246 | `MeasAdd:Period`: Period | toggle (button) | `m.meas; (Period, meas_src)`; catalogue id 13; toggle standard result on W-source A; unit s | Catalogue BASIC > TIMING; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-247 | `MeasAdd:Freq`: Frequency | toggle (button) | `m.meas; (Freq, meas_src)`; catalogue id 14; toggle standard result on W-source A; unit Hz | Catalogue BASIC > TIMING; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-248 | `MeasAdd:Rise`: Rise Time | toggle (button) | `m.meas; (Rise, meas_src)`; catalogue id 21; toggle standard result on W-source A; unit s | Catalogue BASIC > EDGES & COUNTS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-249 | `MeasAdd:Fall`: Fall Time | toggle (button) | `m.meas; (Fall, meas_src)`; catalogue id 22; toggle standard result on W-source A; unit s | Catalogue BASIC > EDGES & COUNTS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-250 | `MeasAdd:PWidth`: Positive Pulse Width | toggle (button) | `m.meas; (PWidth, meas_src)`; catalogue id 17; toggle standard result on W-source A; unit s | Catalogue BASIC > TIMING; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-251 | `MeasAdd:NWidth`: Negative Pulse Width | toggle (button) | `m.meas; (NWidth, meas_src)`; catalogue id 18; toggle standard result on W-source A; unit s | Catalogue BASIC > TIMING; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-252 | `MeasAdd:PDuty`: Positive Duty Cycle | toggle (button) | `m.meas; (PDuty, meas_src)`; catalogue id 27; toggle standard result on W-source A; unit % | Catalogue BASIC > TIMING; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-253 | `MeasAdd:NDuty`: Negative Duty Cycle | toggle (button) | `m.meas; (NDuty, meas_src)`; catalogue id 28; toggle standard result on W-source A; unit % | Catalogue BASIC > TIMING; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-254 | `MeasAdd:PPulses`: Positive Pulses | toggle (button) | `m.meas; (PPulses, meas_src)`; catalogue id 211; toggle standard result on W-source A; unit count/ratio | Catalogue BASIC > EDGES & COUNTS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-255 | `MeasAdd:NPulses`: Negative Pulses | toggle (button) | `m.meas; (NPulses, meas_src)`; catalogue id 212; toggle standard result on W-source A; unit count/ratio | Catalogue BASIC > EDGES & COUNTS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-256 | `MeasAdd:PEdges`: Rising Edges | toggle (button) | `m.meas; (PEdges, meas_src)`; catalogue id 213; toggle standard result on W-source A; unit count/ratio | Catalogue BASIC > EDGES & COUNTS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-257 | `MeasAdd:NEdges`: Falling Edges | toggle (button) | `m.meas; (NEdges, meas_src)`; catalogue id 214; toggle standard result on W-source A; unit count/ratio | Catalogue BASIC > EDGES & COUNTS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-258 | `MeasAdd:TVmax`: Time of Maximum | toggle (button) | `m.meas; (TVmax, meas_src)`; catalogue id 215; toggle standard result on W-source A; unit s | Catalogue BASIC > TIMING; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-259 | `MeasAdd:TVmin`: Time of Minimum | toggle (button) | `m.meas; (TVmin, meas_src)`; catalogue id 216; toggle standard result on W-source A; unit s | Catalogue BASIC > TIMING; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-260 | `MeasAdd:PSlew`: Rising Slew Rate | toggle (button) | `m.meas; (PSlew, meas_src)`; catalogue id 24; toggle standard result on W-source A; unit V/s | Catalogue BASIC > EDGES & COUNTS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |
| menus-261 | `MeasAdd:NSlew`: Falling Slew Rate | toggle (button) | `m.meas; (NSlew, meas_src)`; catalogue id 25; toggle standard result on W-source A; unit V/s | Catalogue BASIC > EDGES & COUNTS; refuses full selection; no eviction; Set available when added | filled when membership active | keep | `ui/src/measures/standard.rs`; `ui/src/ui/meascatalog.rs`; `ui/src/ui/menu/measure.rs` | Changed |

### MeasPair

Path: **Measure catalogue > Settings > Pairs; catalogue > Two sources**. Menu requirement: **menus-18**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/measure.rs`.

Expand all 17 PAIRS in D-Pairs order, W-source A to B. The first eight ids remain here;
indices 8..16 are in Catalogue added measurements and also appear after them in Pairs. Header A/B
pickers reuse menus-219/220 and show off/no-data/empty source status without adding body rows.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-262 | `MeasPair:0`: Delay rise-rise | toggle (button) | `m.meas2`; (0, A, B); Def 20; toggle ordered W-source pair; output s | Pairs dialog index order; catalogue TWO > DELAY; equal sources/full selection rejected | filled when membership active | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs`; `ui/src/ui/tools.rs` | Changed |
| menus-263 | `MeasPair:1`: Delay fall-fall | toggle (button) | `m.meas2`; (1, A, B); Def 230; toggle ordered W-source pair; output s | Pairs dialog index order; catalogue TWO > DELAY; equal sources/full selection rejected | filled when membership active | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs`; `ui/src/ui/tools.rs` | Changed |
| menus-264 | `MeasPair:2`: Delay rise-fall | toggle (button) | `m.meas2`; (2, A, B); Def 231; toggle ordered W-source pair; output s | Pairs dialog index order; catalogue TWO > DELAY; equal sources/full selection rejected | filled when membership active | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs`; `ui/src/ui/tools.rs` | Changed |
| menus-265 | `MeasPair:3`: Delay fall-rise | toggle (button) | `m.meas2`; (3, A, B); Def 232; toggle ordered W-source pair; output s | Pairs dialog index order; catalogue TWO > DELAY; equal sources/full selection rejected | filled when membership active | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs`; `ui/src/ui/tools.rs` | Changed |
| menus-266 | `MeasPair:4`: Phase rise-rise | toggle (button) | `m.meas2`; (4, A, B); Def 23; toggle ordered W-source pair; output ° | Pairs dialog index order; catalogue TWO > PHASE; equal sources/full selection rejected | filled when membership active | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs`; `ui/src/ui/tools.rs` | Changed |
| menus-267 | `MeasPair:5`: Phase fall-fall | toggle (button) | `m.meas2`; (5, A, B); Def 233; toggle ordered W-source pair; output ° | Pairs dialog index order; catalogue TWO > PHASE; equal sources/full selection rejected | filled when membership active | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs`; `ui/src/ui/tools.rs` | Changed |
| menus-268 | `MeasPair:6`: Phase rise-fall | toggle (button) | `m.meas2`; (6, A, B); Def 234; toggle ordered W-source pair; output ° | Pairs dialog index order; catalogue TWO > PHASE; equal sources/full selection rejected | filled when membership active | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs`; `ui/src/ui/tools.rs` | Changed |
| menus-269 | `MeasPair:7`: Phase fall-rise | toggle (button) | `m.meas2`; (7, A, B); Def 235; toggle ordered W-source pair; output ° | Pairs dialog index order; catalogue TWO > PHASE; equal sources/full selection rejected | filled when membership active | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs`; `ui/src/ui/tools.rs` | Changed |

### Dvm

Path: **Menu > Instruments > DVM / counter; Measure > DVM / counter**. Menu requirement: **menus-19**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/dvm.rs`. The ordered read descriptors own menus-270..277;
menus-278 remains the shared source-on injection. `ui/src/ui/menu/dvm/tests.rs` covers every row,
all source-on visibility combinations, live/offline choices, text sizes and picker/Back/Close return.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-270 | `DvmOn`: DVM | toggle (segmented/picker) | `m.dvm_on`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dvm.rs` | Kept |
| menus-271 | `DvmSrc`: DVM source | segmented choice / picker | `m.dvm_src` in live mode: existing CH; when offline, shared Ui.offline_ref with R1..R10 including empty slots | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dvm.rs`; `ui/src/ui/offline.rs` | Changed |
| menus-272 | `DvmMode`: DVM mode | segmented choice / picker | `m.dvm_mode`; DC, AC rms, AC+DC rms | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dvm.rs` | Kept |
| menus-273 | `CntOn`: Counter | toggle (segmented/picker) | `m.cnt_on`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dvm.rs` | Kept |
| menus-274 | `CntSrc`: Count source | segmented choice / picker | `m.cnt_src` in live mode: existing CH; when offline, shared Ui.offline_ref with R1..R10 including empty slots | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dvm.rs`; `ui/src/ui/offline.rs` | Changed |
| menus-275 | `CntMode`: Count mode | segmented choice / picker | `m.cnt_mode`; Frequency, Period, Totalize | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dvm.rs` | Kept |
| menus-276 | `CntDigits`: Digits | numeric | `m.cnt_digits`; 3..7; step 1 | Always | V | keep | `ui/src/ui/menu/dvm.rs` | Kept |
| menus-277 | `CntClear`: Clear counter | action | `counter_clear()`; reset accumulated counter | Always | focus border only | keep | `ui/src/ui/menu/dvm.rs` | Kept |
| menus-278 | `SrcOn`: Switch on source | action | `off_sources(menu); m.ch[c].on`; switch on all missing analogue sources; reset focus | appended only when off_sources(menu) is nonempty; exact source set in recipe below | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |

### Cursor

Path: **Menu > Analysis > Cursor; Cursor key/top bar**. Menu requirement: **menus-20**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/cursor.rs`.

Off: CurMode only. Time/Volt: CurMode, CurA, CurB. Track adds CurSrcB after CurB. XY: all five. Opening via Cursor key or Open(Cursor) changes Off to Time before opening.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-279 | `CurMode`: Mode | segmented choice / picker | `m.cursor`; Off, Time, Volt, Track, XY | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/cursor.rs` | Kept |
| menus-280 | `CurA`: Cursor A | numeric | `m.cur_a`; 0..1 plot fraction; step 0.005; readout physical units when record exists | cursor != Off | V | keep | `ui/src/ui/menu/cursor.rs` | Kept |
| menus-281 | `CurB`: Cursor B | numeric | `m.cur_b`; 0..1, step .005; Track value reads cursor_values()[2] (B time) and uses independent B source | cursor != Off | V | keep | `ui/src/ui/menu/cursor.rs` | Changed |
| menus-282 | `CurC`: Y1 | numeric | `m.cur_c`; 0..1 plot fraction; step 0.005; XY Y1 readout | cursor=XY | V | keep | `ui/src/ui/menu/cursor.rs` | Kept |
| menus-283 | `CurD`: Y2 | numeric | `m.cur_d`; 0..1 plot fraction; step 0.005; XY Y2 readout | cursor=XY | V | keep | `ui/src/ui/menu/cursor.rs` | Kept |
| menus-1221 | `CurSrcB`: Cursor B source | segmented choice / picker | `m.cur_b_source; cursor_b_source/set_cursor_b_source`; W-source; None follows selected workspace source A until B explicitly chosen | CursorMode::Track only | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/cursor.rs` | Added |

### Display

Path: **Menu > Display group > Display; Display top bar**. Menu requirement: **menus-21**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/display.rs`.

TrendSel(0), TrendSel(1) inserted immediately after Windows only when pane=Trend.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-284 | `Open:Windows`: Windows | submenu | `open_menu(Windows); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/windows.rs`; `ui/src/ui/menu/windows.rs` | Kept |
| menus-285 | `TrendSel:0`: Trend chart 1 | segmented choice / picker | `m.trend_sel[0]`; Auto, Off, then active single/pair measurements in measurement order; D-Trend | pane=Trend | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-286 | `TrendSel:1`: Trend chart 2 | segmented choice / picker | `m.trend_sel[1]`; Auto, Off, then active single/pair measurements in measurement order; D-Trend | pane=Trend | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-287 | `Views`: Views | segmented choice / picker | `m.views; views_set`; Off (1), 2, 3, 4 arrangements; distinct from analyser view | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/views.rs`; `ui/src/ui/views.rs` | Kept |
| menus-289 | `Persist`: Persistence | segmented choice / picker | `m.persist; ph.clear`; Min, 100 ms, 500 ms, 1 s, 5 s, 10 s, Infinite | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-290 | `ColorGrade`: Colour grade | toggle (segmented/picker) | `m.color_grade`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-291 | `Intensity`: Intensity | numeric | `m.intensity`; 10..100%; step 5 | Always | V | keep | `ui/src/ui/menu/display.rs` | Kept |
| menus-292 | `GridStyle`: Grid | segmented choice / picker | `m.grid`; Full, Light, None | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-293 | `GridBright`: Grid bright. | numeric | `m.grid_bright`; 10..100%; step 10 | Always | V | keep | `ui/src/ui/menu/display.rs` | Kept |
| menus-294 | `GridFrame`: Grid frame | toggle (segmented/picker) | `m.grid_frame`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-295 | `Dots`: Dots | toggle (segmented/picker) | `m.dots`; Off, On, Auto | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-1222 | `BottomBar`: Bottom bar | segmented choice / picker | `m.bar_compact`; Cards, Compact; relayout/dirty map | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Added |
| menus-296 | `ZInput`: Z input | segmented choice / picker | `m.z_src`; Off + CH | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-297 | `Histogram`: Histogram | segmented choice / picker | `m.hist`; Off, Vertical, Horizontal | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-298 | `XyMode`: Format | toggle (segmented/picker) | `Ui.xy`; YT, XY | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-299 | `Hdmi`: HDMI | segmented choice / picker | `m.hdmi`; Mirror, Waveform, Layout; clamped; Layout warns about second-picture cost; also second Windows row | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-1223 | `HdmiView`: Monitor view | segmented choice / picker | `m.hdmi_view`; Panel's (0), View 1..views; effective only HDMI Layout and >=2 views; otherwise toast | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/monitor.rs`; `ui/src/ui/monitor.rs` | Added |
| menus-300 | `Backlight`: Backlight | numeric | `sys.backlight`; 10..100%; step 5 | Always | V | keep | `ui/src/ui/menu/backlight.rs`; `ui/src/ui/backlight.rs` | Kept |
| menus-301 | `TextSize`: Text size | segmented choice / picker | `m.text_size`; Normal, Large, Larger; clamped step | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs` | Kept |
| menus-302 | `ClearDisp`: Clear | action | `clear_display()`; clear accumulated display; closes Main when invoked there | Always | focus border only | keep | `ui/src/ui/menu/display.rs` | Kept |

### Utility

Path: **Menu > System group > System**. Menu requirement: **menus-22**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/system.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1224 | `Offline`: Record | segmented choice / picker | `Ui.look; set_look`; With live, Take over, Offline; retains/restores live setup and loaded references | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/offline.rs`; `ui/src/ui/offline.rs` | Added |
| menus-303 | `QuickAct`: Quick key | segmented choice / picker | `m.quick`; D-Quick | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/system.rs` | Kept |
| menus-304 | `TouchLock`: Touch | toggle (segmented/picker) | `m.touch_lock`; On (touch enabled), Locked | Removed from this surface: the touch lock has one place on the screen (upstream 9290ca0d) | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | historical: `ui/src/ui.rs` | Dropped |
| menus-305 | `AuxOut`: AUX out | segmented choice / picker | `m.aux_out; aux_adjust`; Trigger out, Off; Pass/fail if board hardware mask or zone capability | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/hwmask.rs`; `ui/src/ui/menu/system.rs` | Kept |
| menus-306 | `BootMode`: Boot mode | segmented choice / picker | `boot_mode(); boot_mode_toggle`; One-shot, Persist | Always; rejects while update job busy | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/system.rs` | Kept |
| menus-307 | `Overclock`: Overclock | segmented choice / picker | `Ui.clock_next; overclock_step`; D-Clock, board-supported clock steps | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/system.rs` | Kept |
| menus-308 | `Graphics`: Graphics | segmented choice / picker | `sys.gpu; graphics_adjust`; CPU, GPU (choice; runtime fallback reported separately) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/graphics.rs`; `ui/src/ui/graphics.rs` | Kept |
| menus-309 | `Shot`: Screenshot | action | `Ui.shot_request`; close menu and save screenshot after next frame | Always | focus border only | keep | `ui/src/ui/menu/system.rs` | Kept |
| menus-310 | `Open:SelfCal`: Self-cal | submenu | `open_menu(SelfCal); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/selfcal.rs`; `ui/src/ui/menu/calibration.rs` | Kept |
| menus-311 | `Open:Network`: Network | submenu | `open_menu(Network); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/menu/network.rs` | Kept |
| menus-1225 | `Open:Features`: Features | submenu | `open_submenu(Features)`; settings for offered features and decoders | Always | focus only; no summary | change H1: inline summary | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-312 | `Open:Time`: Date and time | submenu | `open_menu(Time); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/time.rs`; `ui/src/ui/menu/time.rs` | Kept |
| menus-313 | `SysInfo`: System info | action | `sysinfo_open()`; show system-information dialog | Always | focus border only | keep | `ui/src/ui/menu/system.rs` | Kept |
| menus-314 | `About`: About OOS | action | `Ui.dialog`; show build/platform/source/about dialog | Always | focus border only | keep | `ui/src/ui/menu/system.rs` | Kept |
| menus-315 | `Update`: Update | action | `update_lines(); Ui.dialog`; show update instructions/status | Always | focus border only | keep | `ui/src/ui/menu/system.rs` | Kept |
| menus-316 | `UpdateRollback`: Roll back update | action | `update_rollback_tap()`; two taps within 5 s; reject busy update or missing/already-current rollback image | Always | focus border only | keep | `ui/src/ui/menu/system.rs` | Kept |
| menus-317 | `Stock`: Back to stock | action | `stock_tap()`; show power-cycle guidance; Persist requires changing to One-shot first | Always | focus border only | keep | `ui/src/ui/menu/system.rs` | Kept |

### Math

Path: **Menu > Analysis > Math; Math key**. Menu requirement: **menus-23**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/math.rs`.

Off: MathSel, MathFam, MathOpI. Expr: MathSel, MathFam, MathOpI, MathExprI, MathExprEdit, MathScale, MathOffset, MathAuto. Otherwise: MathSel, MathFam, MathOpI, MathA; MathB if two_sources; Lin A/B if Lin; Fc if has_fc; Bw if has_bw; MathScale, MathOffset; FftWin if FFT; MathAuto.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-318 | `MathSel`: Channel | segmented choice / picker | `m.math_sel; math_select`; M1..M8 | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/math.rs` | Kept |
| menus-1226 | `MathFam`: Category | segmented choice / picker | `Ui.math_fam`; Arithmetic, Function, Filter, Logic, Convert; filters operation choices, leaves current operation unchanged | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/math.rs` | Added |
| menus-319 | `MathOpI`: Operation | segmented choice / picker | `m.math`; Off followed by operations in Ui.math_fam, D-Math; changing family changes offered operations without changing the current operation | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/math.rs` | Changed |
| menus-320 | `MathExprI`: Formula | status (text only) | `m.math_expr[m.math_sel]`; read-only formula string or none | math=Expr; text only | text only | keep | `ui/src/ui/menu/math.rs` | Kept |
| menus-321 | `MathExprEdit`: Edit formula | action | `m.math_expr[math_sel]; Osk::for_expr`; formula keyboard | math=Expr | focus border only | keep | `ui/src/ui/menu/math.rs` | Kept |
| menus-322 | `MathA`: Source A | segmented choice / picker | `m.math_a`; existing CH, earlier M banks, then loaded reference indices after M8; preserve dependency order; math_source_ids | math neither Off nor Expr | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/math.rs` | Changed |
| menus-323 | `MathB`: Source B | segmented choice / picker | `m.math_b`; existing CH, earlier M banks, then loaded reference indices after M8; preserve dependency order; math_source_ids | math.two_sources() | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/math.rs` | Changed |
| menus-324 | `MathLinA`: A | numeric | `m.math_lin.0`; -1e6..1e6; step 0.1 | math=Lin | V | keep | `ui/src/ui/menu/math.rs` | Kept |
| menus-325 | `MathLinB`: B | numeric | `m.math_lin.1`; -1e6..1e6; step 0.1 | math=Lin | V | keep | `ui/src/ui/menu/math.rs` | Kept |
| menus-326 | `MathFc`: Frequency | numeric | `m.math_fc`; Hz; reciprocal trigger time ladder 1 ns..10 s; reverse step | math.has_fc() | V | keep | `ui/src/ui/menu/math.rs` | Kept |
| menus-327 | `MathBw`: Bandwidth | numeric | `m.math_bw`; Hz; reciprocal trigger time ladder 1 ns..10 s; reverse step | math.has_bw() | V | keep | `ui/src/ui/menu/math.rs` | Kept |
| menus-328 | `MathScale`: Scale | numeric | `m.math_scale`; FFT 1, 2, 5, 10, 20 dB/div; otherwise math_scales(), D-MathScale | math != Off | V | keep | `ui/src/ui/menu/math.rs` | Kept |
| menus-329 | `MathOffset`: Offset | numeric | `m.math_offset`; div; step 0.1; no local clamp | math != Off | V | keep | `ui/src/ui/menu/math.rs` | Kept |
| menus-330 | `FftWin`: Window | segmented choice / picker | `m.fft_win`; Rect, Hann, Hamming, Blackman, Flattop | math=FFT | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/math.rs` | Kept |
| menus-331 | `MathAuto`: Auto scale | action | `math_autoscale()`; autoscale selected math | math != Off | focus border only | keep | `ui/src/ui/menu/math.rs` | Kept |

### Ref

Path: **Menu > Analysis > Reference; Ref key**. Menu requirement: **menus-24**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/reference.rs`.

RefShow expands R1..R10 after Import; selected empty-slot adjustments do nothing and slot name reports an error.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-332 | `RefSrc`: Save from | segmented choice / picker | `Ui.ref_src`; CH | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Changed |
| menus-333 | `RefSave`: Save to next R | action | `last[0]; Ui.refs`; save source into first empty R, else R10; reject missing source data | Always | focus border only | keep | `ui/src/ui/menu/reference.rs` | Kept |
| menus-334 | `RefClear`: Clear all | action | `Ui.refs`; clear all 10 reference slots, recompute reference measurements and analysers | Always | focus border only | keep | `ui/src/ui/menu/reference.rs` | Changed |
| menus-335 | `RefSel`: Slot | picker (stepped choice today) | `Ui.ref_sel; select_workspace_source(Ref(slot))`; R1..R10; also updates measurement A/knob context | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Changed |
| menus-336 | `RefVpd`: Slot scale | numeric | `Ui.refs[ref_sel].vpd`; D-VScale; V/div; clamped | always; empty selected slot does nothing | V | keep | `ui/src/ui/menu/reference.rs` | Kept |
| menus-337 | `RefOffset`: Slot offset | numeric | `Ui.refs[ref_sel].offset`; V; step 0.1 slot V/div | always; empty selected slot does nothing | V | keep | `ui/src/ui/menu/reference.rs` | Kept |
| menus-338 | `RefColor`: Slot color | picker (stepped choice today) | `Ui.refs[ref_sel].color`; White, Red, Green, Blue, Violet; wraps | always; empty selected slot does nothing | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-339 | `RefLabel`: Slot name | action | `Ui.refs[ref_sel].label; Osk::for_ref`; name keyboard; toast if slot empty | Always | focus border only | keep | `ui/src/ui/menu/reference.rs` | Kept |
| menus-1227 | `RefLoad`: Load file | action | `ref_load / ref_import`; takes the file from storage into the Slot row's reference, as Import slot does; loaded record overlays live acquisition by default | load feature offered | focus border only | keep | `ui/src/ui/menu/reference.rs`; `ui/src/ui/tools.rs`; `ref_load` | Changed: the destination chooser and host file picker moved to oos-desktop (ADR 0027) |
| menus-340 | `RefExport`: Export slot | action | `ref_export()`; export selected reference slot | Always | focus border only | keep | `ui/src/ui/menu/reference.rs` | Kept |
| menus-341 | `RefImport`: Import slot | action | `ref_import()`; import selected reference slot | Always | focus border only | keep | `ui/src/ui/menu/reference.rs` | Kept |
| menus-342 | `RefShow:0`: R1 | toggle (button) | `Ui.refs[0].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-343 | `RefShow:1`: R2 | toggle (button) | `Ui.refs[1].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-344 | `RefShow:2`: R3 | toggle (button) | `Ui.refs[2].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-345 | `RefShow:3`: R4 | toggle (button) | `Ui.refs[3].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-346 | `RefShow:4`: R5 | toggle (button) | `Ui.refs[4].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-347 | `RefShow:5`: R6 | toggle (button) | `Ui.refs[5].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-348 | `RefShow:6`: R7 | toggle (button) | `Ui.refs[6].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-349 | `RefShow:7`: R8 | toggle (button) | `Ui.refs[7].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-350 | `RefShow:8`: R9 | toggle (button) | `Ui.refs[8].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |
| menus-351 | `RefShow:9`: R10 | toggle (button) | `Ui.refs[9].on`; empty or CHn on/off; toggle this populated slot | always; empty slots do nothing | filled while populated slot is On; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/reference.rs` | Kept |

### Decode

Path: **Menu > Analysis > Decode; Analyse key**. Menu requirement: **menus-25**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/decode.rs`.

Runtime order follows the decoder recipe below. Decoder selection switches bank, not a protocol in every bank.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-352 | `DecSel`: Decoder no. | segmented choice / picker | `m.dec_sel; dec_select`; D1, D2, D3, D4; selected decoder bank | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs` | Kept |
| menus-353 | `DecOn`: Decoder | toggle (segmented/picker) | `m.dec_on`; Off, On for selected decoder | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs` | Kept |
| menus-354 | `DecProto`: Protocol | segmented choice / picker | `m.dec_proto; set_dec_proto`; enabled D-Protocols registry. Existing reference-source context sets protocol and only applies la_off threshold/input override. Otherwise hardware preserves shared channels/source/trigger and allocates new roles from free on channels; demo applies module setup and enables needed sources. Newly changed non-reference protocol resets own options and may open Bus view pane. | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs` | Changed |
| menus-355 | `DecIn`: Input | toggle (segmented/picker) | `m.dec_la_in`; Analog, LA | la_on() and protocol is not C-PHY | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs` | Changed |
| menus-356 | `DecSrc`: Source | segmented choice / picker | `m.dec_src; configure_decoder_source` for CH/R, `m.dec_la[0]; la_adjust` for LA; D-decoder-sources; analogue/loaded reference or LA line by input context; role label from module | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/ui/reference_decode.rs` | Changed |
| menus-357 | `DecSrc2`: Clock | segmented choice / picker | `m.dec_src2; configure_decoder_source` for CH/R, `m.dec_la[1]; la_adjust` for LA; D-decoder-sources; analogue/loaded reference or LA line by input context; role label from module | protocol role[1] exists | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/ui/reference_decode.rs` | Changed |
| menus-358 | `DecCs`: CS | segmented choice / picker | `m.dec_cs; configure_decoder_source` for CH/R, `m.dec_la[2]; la_adjust` for LA; D-decoder-sources; None prefix; C-PHY C third role is required (no None); analogue/loaded reference or LA line by input context; role label from module | protocol role[2] exists | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/ui/reference_decode.rs` | Changed |
| menus-359 | `DecSrc4`: Source 4 | segmented choice / picker | `m.dec_src4; configure_decoder_source` for CH/R, `m.dec_la[3]; la_adjust` for LA; D-decoder-sources; None prefix; analogue/loaded reference or LA line by input context; role label from module | protocol role[3] exists | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/ui/reference_decode.rs` | Changed |
| menus-360 | `DecDiff`: B line (A-B) | segmented choice / picker | `m.dec_diff; configure_decoder_source`; D-decoder-sources; None prefix; analogue/loaded reference or LA line by input context; role label from module | protocol diff_ok() and analogue input | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/ui/reference_decode.rs` | Changed |
| menus-361 | `DecBaud`: Baud | segmented choice / picker | `m.dec_baud`; BAUDS (D-Baud) | Only when selected protocol declares corresponding Protocol::shared row; registry order below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-362 | `DecBits`: Data bits | numeric | `m.dec_bits`; UART 5..9; other protocols 4..32; step 1 | Only when selected protocol declares corresponding Protocol::shared row; registry order below | V | keep | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-363 | `DecParity`: Parity | segmented choice / picker | `m.dec_parity`; None, Even, Odd | Only when selected protocol declares corresponding Protocol::shared row; registry order below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-364 | `DecFmt`: Format | toggle (segmented/picker) | `m.dec_ascii`; Hex, ASCII | Only when selected protocol declares corresponding Protocol::shared row; registry order below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-365 | `DecMode`: Mode | segmented choice / picker | `m.dec_mode`; 0, 1, 2, 3 | Only when selected protocol declares corresponding Protocol::shared row; registry order below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-366 | `DecCanRate`: Bit rate | segmented choice / picker | `m.dec_canrate`; CAN_RATES (D-CAN) | Only when selected protocol declares corresponding Protocol::shared row; registry order below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-367 | `DecCanH`: Signal | toggle (segmented/picker) | `m.dec_canh`; RX/TX, CAN_H | Only when selected protocol declares corresponding Protocol::shared row; registry order below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-368 | `DecFdRate`: Data rate | segmented choice / picker | `m.dec_fdrate`; 1, 2, 4, 5, 8 Mbit/s | Removed from this surface: DecFdRate replaced by DecOpt module row(s), listed below | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | e9f7c30b historical: `ui/src/ui.rs`; `ui/src/ui.rs`; current constructor `ui/src/ui.rs` | Dropped |
| menus-369 | `DecArate`: Bit rate | segmented choice / picker | `m.dec_arate`; Auto, 12.5 k, 100 kbit/s | Removed from this surface: DecArate replaced by DecOpt module row(s), listed below | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | e9f7c30b historical: `ui/src/ui.rs`; `ui/src/ui.rs`; current constructor `ui/src/ui.rs` | Dropped |
| menus-370 | `DecTick`: Clock tick | segmented choice / picker | `m.dec_tick`; SENT_TICKS (D-SENT), us | Removed from this surface: DecTick replaced by DecOpt module row(s), listed below | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | e9f7c30b historical: `ui/src/ui.rs`; `ui/src/ui.rs`; current constructor `ui/src/ui.rs` | Dropped |
| menus-371 | `DecNibbles`: Data nibbles | segmented choice / picker | `m.dec_nibbles`; 1..6; step 1 | Removed from this surface: DecNibbles replaced by DecOpt module row(s), listed below | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | e9f7c30b historical: `ui/src/ui.rs`; `ui/src/ui.rs`; current constructor `ui/src/ui.rs` | Dropped |
| menus-372 | `DecUsbSpeed`: Speed | segmented choice / picker | `m.dec_uspeed`; Auto, Low 1.5M, Full 12M | Only when selected protocol declares corresponding Protocol::shared row; registry order below | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-1228 | `DecCphyRate`: Symbol rate | picker | `m.dec_cphy_rate`; Auto, then 0.5,1,1.5,2,2.25,2.5,3,3.5,4,4.5,6,8,10,12,16,20,25,32 Gsym/s; cycles 19 CPHY_RATES | C-PHY declares Row::CphyRate | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/decode.rs`; `ui/src/ui/menu/decode.rs` | Added |
| menus-373 | `DecOpt`: Option | segmented choice / picker | `m.dec_opt[k] / m.dec_opts()[k]`; module option index cycles Opt.choices; live production rows and explicit domains listed below | One row per selected module option, in declaration order, after shared rows | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs` | Changed |
| menus-374 | `DecThr`: Threshold | numeric | `m.dec_thr`; step decoder_vpd()/25 V; no local clamp; C-PHY label Min level gap | analogue input | V | keep | `ui/src/ui/menu/decode.rs` | Changed |
| menus-375 | `DecTable`: Event table | action | `table_toggle(Decode)`; open/close decoded event table | Always | focus border only | keep | `ui/src/ui/menu/decode.rs` | Kept |
| menus-376 | `DecExport`: Export events | action | `events_export(Decode)`; export decoded event records | Always | focus border only | keep | `ui/src/ui/menu/decode.rs` | Kept |
| menus-377 | `SrcOn`: Switch on source | action | `off_sources(menu); m.ch[c].on`; switch on all missing analogue sources; reset focus | appended only when off_sources(menu) is nonempty; exact source set in recipe below | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |

### Storage

Path: **Menu > System group > Storage; Storage top bar/USB box**. Menu requirement: **menus-26**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/storage.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1229 | `StoreDest`: Save to | segmented choice / picker | `Ui.store_dest`; Auto, USB, Local; Auto selects mounted stick else local; explicit USB errors remain visible | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/storage.rs` | Added |
| menus-378 | `StoreSlot`: Setup slot | picker (stepped slot today) | `Ui.store_slot`; 1..9; step 1; setupN.txt | Always | V; committed slot value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/storage.rs` | Kept |
| menus-379 | `StoreSaveSetup`: Save setup | action | `lifecycle::storage::save`; bounded worker saves selected setup slot and reports admission/completion errors | Always | focus border only | keep | `ui/src/ui/menu/storage.rs` | Kept |
| menus-380 | `StoreRecall`: Recall setup | action | `recall_setup()`; recall selected slot; retain run state/text-size preference; report parse/IO errors | Always | focus border only | keep | `ui/src/ui/menu/storage.rs` | Kept |
| menus-381 | `StoreCsv`: Save waveform (CSV) | action | `last[0]; store::wave_csv/la_csv`; save waveform and separate LA changes if active; reject no waveform | Always | focus border only | keep | `ui/src/ui/menu/storage.rs` | Kept |
| menus-382 | `StoreBin`: Save waveform (binary) | action | `last[0]; store::wave_bin/write`; save binary waveform; toast if no record | Always | focus border only | keep | `ui/src/ui/menu/storage.rs` | Kept |
| menus-1230 | `StoreFfSave`: Save fast-frame | action | `ff_save()`; save history, else last frame; rejects no frames | Always | focus border only | keep | `ui/src/ui/menu/storage.rs` | Added |
| menus-1231 | `StoreFfLoad`: Load fast-frame | action | `ff_load()`; latest frame file in selected save destination; replace history and stop at newest frame | Always | focus border only | keep | `ui/src/ui/menu/storage.rs` | Added |
| menus-383 | `StorePng`: Save screenshot (PNG) | action | `Ui.shot_request`; close menu then save PNG | Always | focus border only | keep | `ui/src/ui/menu/storage.rs` | Kept |

### Mask

Path: **Menu > Analysis > Mask**. Menu requirement: **menus-27**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/mask.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-384 | `MaskOn`: Mask test | toggle (segmented/picker) | `m.mask_on; create_mask`; Off, On; creates mask if absent when enabling | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/mask.rs` | Kept |
| menus-385 | `MaskSrc`: Source | segmented choice / picker | `m.mask_src`; CH; changing source clears mask/reference and disables test | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/mask.rs` | Kept |
| menus-1232 | `MaskInvert`: Invert | toggle | `m.mask_inv`; Off, On; invert pass/fail sense | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/mask.rs` | Added |
| menus-386 | `MaskX`: Tolerance X | numeric | `m.mask_x`; 0..2 div; step 0.04; rebuild mask | Always | V | keep | `ui/src/ui/menu/mask.rs` | Kept |
| menus-387 | `MaskY`: Tolerance Y | numeric | `m.mask_y`; 0.04..4 div; step 0.04; rebuild mask | Always | V | keep | `ui/src/ui/menu/mask.rs` | Kept |
| menus-388 | `MaskAction`: On fail | toggle (segmented/picker) | `m.mask_stop`; None, Stop | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/mask.rs` | Kept |
| menus-389 | `MaskCreate`: Create from CH | action | `create_mask()`; create mask from current source waveform | Always | focus border only | keep | `ui/src/ui/menu/mask.rs` | Changed |
| menus-1233 | `MaskFromRef`: Create from ref | action | `create_mask_from_ref()`; selected ref, rejects empty; sets mask source, enables test and resets counts | Always | focus border only | keep | `ui/src/ui/menu/mask.rs` | Added |
| menus-390 | `MaskReset`: Reset counts | action | `change::mask_reset`; reset pass/fail counts | Always | focus border only | keep | `ui/src/ui/menu/mask.rs` | Kept |
| menus-1234 | `MaskClear`: Clear mask | action | clear mask/reference data, disable test, zero pass/fail counts | Always | focus border only | keep | `ui/src/ui/menu/mask.rs` | Added |
| menus-391 | `MaskSave`: Save mask | action | `mask_save()`; save mask | Always | focus border only | keep | `ui/src/ui/menu/mask.rs` | Kept |
| menus-392 | `MaskLoad`: Load mask | action | `mask_load()`; load mask | Always | focus border only | keep | `ui/src/ui/menu/mask.rs` | Kept |
| menus-393 | `SrcOn`: Switch on source | action | `off_sources(menu); m.ch[c].on`; switch on all missing analogue sources; reset focus | appended only when off_sources(menu) is nonempty; exact source set in recipe below | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |

### Zone

Path: **Menu > Trigger group > Zones; Trigger > Zones**. Menu requirement: **menus-28**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/zone.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-394 | `ZoneSel`: Zone | segmented choice / picker | `Ui.zone_sel`; Z1, Z2 | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/zone.rs` | Kept |
| menus-395 | `ZoneOn`: Enable | toggle (segmented/picker) | `m.zones[zone_sel].on`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/zone.rs` | Kept |
| menus-396 | `ZoneSrc`: Source | segmented choice / picker | `m.zones[zone_sel].src`; CH | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/zone.rs` | Kept |
| menus-397 | `ZoneMode`: Condition | toggle (segmented/picker) | `m.zones[zone_sel].inside`; Intersect, Not intersect | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/zone.rs` | Kept |
| menus-398 | `ZoneBox`: Draw box | action | `Ui.zone_arm`; close menu; next one-finger drag draws box | Always | focus border only | keep | `ui/src/ui/menu/zone.rs` | Kept |
| menus-399 | `ZoneShape`: Draw shape | action | `Ui.zone_arm`; close menu; next one-finger drag draws polygon | Always | focus border only | keep | `ui/src/ui/menu/zone.rs` | Kept |
| menus-400 | `ZoneClear`: Delete | action | `m.zones[zone_sel]`; delete geometry/enable state, retain source | Always | focus border only | keep | `ui/src/ui/menu/zone.rs` | Kept |

### Spectrum

Path: **Menu > Analysers > Spectrum; active Spectrum overflow > Settings**. Menu requirement: **menus-29**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/spectrum.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-401 | `SpecOn`: View | toggle (segmented/picker) | `m.view`; Off (Scope), On (Spectrum) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-402 | `SpecSrc`: Source | segmented choice / picker | `m.spec_src; step_wave`; W-source (existing CH then loaded R); changing Eye source updates rate if from-record and clears eye caches | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Changed |
| menus-403 | `SpecCenter`: Centre | numeric | `m.spec_center`; F-ladder Hz; 0 allowed | Always | V | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-404 | `SpecSpan`: Span | numeric | `m.spec_span`; F-ladder Hz; 0 = Full/Auto effective span | Always | V | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-405 | `SpecHarm`: Harmonics | segmented choice / picker | `m.spec_harm`; D-Harmonics | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-406 | `SpecRbw`: RBW | numeric | `m.spec_rbw`; F-ladder Hz; 0 = Auto, display computed RBW if available | Always | V | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-407 | `SpecWin`: Window | segmented choice / picker | `m.spec_win`; Rect, Hann, Hamming, Blackman, Flattop | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-408 | `SpecTrace`: Trace | segmented choice / picker | `m.spec_trace`; Normal, Average, Max hold, Min hold | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-409 | `SpecRef`: Ref level | numeric | `m.spec_ref`; dBV; step 5; no local clamp | Always | V | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-410 | `SpecWf`: Waterfall | toggle (segmented/picker) | `m.spec_wf`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-411 | `SpecDist`: THD / SNR | toggle (segmented/picker) | `m.spec_dist`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-1235 | `SpecAcprW`: Ch width | numeric | `m.spec_acpr_w`; F-ladder Hz, min 1 Hz; starts from max(current,1) | Always | V | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1236 | `SpecAcprS`: Ch space | numeric | `m.spec_acpr_s`; F-ladder Hz, min 1 Hz; starts from max(current,1) | Always | V | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-412 | `SpecPeak`: Centre on peak | action | `strongest_line(); m.spec_center/spec_span`; centre on strongest peak; initialize span if full | Always | focus border only | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-413 | `SpecMargin`: Limit margin | numeric | `m.spec_margin`; 0..60 dB; step 1 | Always | V | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-414 | `SpecLimitSet`: Limit from trace | action | `Ui.azr.limit; spectrum::limit_from`; capture limit from current spectrum + margin; reject missing spectrum | Always | focus border only | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-415 | `SpecLimitClear`: Clear limit | action | `Ui.azr.limit`; clear spectrum limit | Always | focus border only | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-416 | `SrcOn`: Switch on source | action | `off_sources(menu); m.ch[c].on`; switch on all missing analogue sources; reset focus | appended only when off_sources(menu) is nonempty; exact source set in recipe below | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |

### Eye

Path: **Menu > Analysers > Eye / jitter; active Eye overflow > Settings**. Menu requirement: **menus-30**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/eye.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1237 | `EyeAct`: Auto Eye/Jitter | action | `eye_make()`; detect rate from record and open eye | Always | focus border only | keep | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-417 | `EyeOn`: View | toggle (segmented/picker) | `m.view`; Off (Scope), On (Eye) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Changed |
| menus-1238 | `EyePlace`: Place | segmented choice / picker | `m.eye_place; eye_set_place`; Full, Side, Under; returns floating eye to chosen placement | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-418 | `EyeSrc`: Source | segmented choice / picker | `m.eye_src; step_wave`; W-source (existing CH then loaded R); changing Eye source updates rate if from-record and clears eye caches | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Changed |
| menus-419 | `EyeRate`: Bit rate | numeric | `m.eye_rate; m.eye_from_record`; F-ladder b/s; zero Auto; manual adjustment clears from-record flag; detected rate reads with from record suffix | Always | V | keep | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Changed |
| menus-1239 | `EyeBer`: BER | numeric | `m.eye_ber`; 1e-3,1e-6,1e-9,1e-12,1e-15; clamped | Always | V | keep | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1240 | `EyeB`: Clock | segmented choice / picker | `m.eye_b`; Off then W-source | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1241 | `EyeHp`: Highpass | toggle | `m.eye_hp`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1242 | `EyePnLo`: Pn low | numeric | `m.eye_pn_lo`; F-ladder Hz, minimum 1 | Always | V | keep | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1243 | `EyePnHi`: Pn high | numeric | `m.eye_pn_hi`; F-ladder Hz, minimum 1 | Always | V | keep | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1244 | `EyeNpj`: NPJ | toggle | `m.eye_npj`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1245 | `EyeRail`: Rail | segmented choice / picker | `m.eye_rail`; Off then W-source | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1246 | `EyeNotch`: Notch | numeric | `m.eye_notch`; F-ladder Hz; zero off | Always | V | keep | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-420 | `EyeClear`: Clear eye | action | `Ui.azr.eye/eye_res/page`; clear eye accumulation, retain active view | Always | focus border only | keep | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-421 | `SrcOn`: Switch on source | action | `off_sources(menu); m.ch[c].on`; switch on all missing analogue sources; reset focus | appended only when off_sources(menu) is nonempty; exact source set in recipe below | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |

### Power

Path: **Menu > Analysers > Power analysis; active Power overflow > Settings**. Menu requirement: **menus-31**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/power.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-422 | `PowOn`: View | toggle (segmented/picker) | `m.view`; Off (Scope), On (Power) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysis.rs` | Changed |
| menus-423 | `PowV`: Voltage | segmented choice / picker | `m.pw_v; step_wave`; W-source (existing CH then loaded R); changing Eye source updates rate if from-record and clears eye caches | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysis.rs` | Changed |
| menus-424 | `PowI`: Current | segmented choice / picker | `m.pw_i; step_wave`; W-source (existing CH then loaded R); changing Eye source updates rate if from-record and clears eye caches | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysis.rs` | Changed |
| menus-425 | `PowMode`: Page | segmented choice / picker | `m.pw_mode`; Quality, Harmonics, Switching, Ripple, Cin, Rail in POWER_MODES order | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysis.rs` | Changed |
| menus-1247 | `PowFline`: Line f | numeric | `m.pw_fline`; F-ladder Hz; zero auto | Ripple only | V | keep | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1248 | `PowFsw`: Switch f | numeric | `m.pw_fsw`; F-ladder Hz; zero auto | Ripple only | V | keep | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1250 | `PowRef`: Reference | numeric | `m.pw_ref`; Top(None), else positive F-ladder V | Rail only; before Input | V | keep | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1249 | `PowVin`: Input | segmented choice / picker | `m.pw_vin`; Off then W-source | Ripple or Rail | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-1251 | `PowClk`: Clock | segmented choice / picker | `m.pw_clk`; Off then W-source | Rail only | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/power.rs`; `ui/src/ui/analysis.rs` | Added |
| menus-426 | `SrcOn`: Switch on source | action | `off_sources(Power)`; Ripple/Rail require V only, other pages V+I; loaded reference slots do not require analogue enable | appended only when off_sources(menu) is nonempty; exact source set in recipe below | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Changed |

### Identify

Path: **Menu > Analysers > Identify (runs identification before opening)**. Menu requirement: **menus-32**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/identify.rs`.

ProtoWatch then findings 0..min(findings.len(),8)-1. IdentifyAct is the entry action rather than Open(Identify).

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-427 | `ProtoWatch`: Protocol watch | toggle (segmented/picker) | `Ui.azr.watch`; Off, On; clears watch history | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/identify.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-428 | `IdFind:0`: Finding 1 | action (legacy Choice) | `Ui.azr.findings[0]; apply_finding`; first 8 findings; applies source/trigger/decoder and closes menu | finding exists; first eight only | sole option equals value, so filled whenever segmented | keep (action, no persistent option) | `ui/src/ui/menu/identify.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-429 | `IdFind:1`: Finding 2 | action (legacy Choice) | `Ui.azr.findings[1]; apply_finding`; first 8 findings; applies source/trigger/decoder and closes menu | finding exists; first eight only | sole option equals value, so filled whenever segmented | keep (action, no persistent option) | `ui/src/ui/menu/identify.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-430 | `IdFind:2`: Finding 3 | action (legacy Choice) | `Ui.azr.findings[2]; apply_finding`; first 8 findings; applies source/trigger/decoder and closes menu | finding exists; first eight only | sole option equals value, so filled whenever segmented | keep (action, no persistent option) | `ui/src/ui/menu/identify.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-431 | `IdFind:3`: Finding 4 | action (legacy Choice) | `Ui.azr.findings[3]; apply_finding`; first 8 findings; applies source/trigger/decoder and closes menu | finding exists; first eight only | sole option equals value, so filled whenever segmented | keep (action, no persistent option) | `ui/src/ui/menu/identify.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-432 | `IdFind:4`: Finding 5 | action (legacy Choice) | `Ui.azr.findings[4]; apply_finding`; first 8 findings; applies source/trigger/decoder and closes menu | finding exists; first eight only | sole option equals value, so filled whenever segmented | keep (action, no persistent option) | `ui/src/ui/menu/identify.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-433 | `IdFind:5`: Finding 6 | action (legacy Choice) | `Ui.azr.findings[5]; apply_finding`; first 8 findings; applies source/trigger/decoder and closes menu | finding exists; first eight only | sole option equals value, so filled whenever segmented | keep (action, no persistent option) | `ui/src/ui/menu/identify.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-434 | `IdFind:6`: Finding 7 | action (legacy Choice) | `Ui.azr.findings[6]; apply_finding`; first 8 findings; applies source/trigger/decoder and closes menu | finding exists; first eight only | sole option equals value, so filled whenever segmented | keep (action, no persistent option) | `ui/src/ui/menu/identify.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-435 | `IdFind:7`: Finding 8 | action (legacy Choice) | `Ui.azr.findings[7]; apply_finding`; first 8 findings; applies source/trigger/decoder and closes menu | finding exists; first eight only | sole option equals value, so filled whenever segmented | keep (action, no persistent option) | `ui/src/ui/menu/identify.rs`; `ui/src/ui/analysis.rs` | Kept |

### SelfCal

Path: **System > Self-cal**. Menu requirement: **menus-33**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/calibration.rs`.

Exactly one of Start (not running) or Cancel (running). Extraction records legacy behaviour only; no calibration operation is authorized by this document.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-436 | `CalCh`: Channels | segmented choice / picker | `Ui.cal.ch`; All then physically existing CH1..CHn; cycles n_ch()+1 | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/calibration.rs`; `ui/src/ui/selfcal.rs` | Changed |
| menus-437 | `CalAdc`: ADC cores | toggle (segmented/picker) | `Ui.cal.adc`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/calibration.rs`; `ui/src/ui/selfcal.rs` | Kept |
| menus-438 | `CalZero`: AFE zero | toggle (segmented/picker) | `Ui.cal.zero`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/calibration.rs`; `ui/src/ui/selfcal.rs` | Kept |
| menus-439 | `CalStart`: Start | action | `Ui.cal.armed; shared.cal_req`; two Start presses within 10 s after disconnect warning; hardware required | not cal_running(); hardware required to start | focus border only | keep | `ui/src/ui/menu/calibration.rs`; `ui/src/ui/selfcal.rs` | Kept |
| menus-440 | `CalCancel`: Cancel | action | `shared.cal_cancel`; request running calibration cancellation | cal_running() | focus border only | keep | `ui/src/ui/menu/calibration.rs`; `ui/src/ui/selfcal.rs` | Kept |
| menus-441 | `CalStatus`: Status | status (text only) | `cal_status()`; read-only idle/starting/progress/result, or hardware required | Always | text only | keep | `ui/src/ui/menu/calibration.rs`; `ui/src/ui/selfcal.rs` | Kept |

### Network

Path: **System > Network; network top-bar box**. Menu requirement: **menus-34**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/network.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-442 | `NetEth`: Ethernet | status (text only) | `net_value(NetEth)`; read-only Ethernet link/address/rate text | Always | text only | keep | `ui/src/ui/wlan.rs`; `ui/src/ui/menu/network.rs` | Kept |
| menus-443 | `NetRetry10`: 10 Mbit retry | toggle (segmented/picker) | `sys.eth_retry_10m`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/time.rs`; `ui/src/ui/time.rs` | Kept |
| menus-444 | `NetMac`: MAC | status (text only) | `net_value(NetMac)`; read-only MAC text or - | Always | text only | keep | `ui/src/ui/wlan.rs`; `ui/src/ui/menu/network.rs` | Kept |
| menus-445 | `Open:Remote`: Remote services | submenu | `open_menu(Remote); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/remote.rs`; `ui/src/ui/menu/remote.rs` | Kept |
| menus-1252 | `NetPorts`: Other access | status | `net_value(NetPorts)`; Windows uses published lan_status; unit displays fixed SCPI/VXI-11/SSH description (may differ from enabled services) | Always | text only | keep | `ui/src/ui/menu/network.rs`; `ui/src/ui/wlan.rs` | Added |
| menus-446 | `NetWlan`: WLAN | status (text only) | `net_value(NetWlan)`; read-only WLAN state/address text | Always | text only | keep | `ui/src/ui/menu/network.rs` | Kept |
| menus-447 | `WlanScan`: Find WLAN networks | action | `wlan_scan()`; scan asynchronously; show network picker | Always | focus border only | keep | `ui/src/ui/menu/network.rs` | Kept |
| menus-448 | `WlanForget`: Forget WLAN network | action | `wlan_forget_tap()`; show saved-network removal picker | Always | focus border only | keep | `ui/src/ui/menu/network.rs` | Kept |
| menus-449 | `WlanInfo`: WLAN details | action | `wlan_lines(); Ui.dialog`; show WLAN details | Always | focus border only | keep | `ui/src/ui/menu/network.rs` | Kept |

### Remote

Path: **Network > Remote services**. Menu requirement: **menus-35**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/remote.rs`.

Stored order is column-major. Display pairs by row: Web/Address, SCPI/Port, VXI-11/Port, telnet/Port, SSH/SSH settings, mDNS/Name, Allowed/Firewall.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-450 | `RemWeb`: Web remote | toggle (segmented/picker) | `sys.remote_web`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Kept |
| menus-451 | `RemScpi`: SCPI | toggle (segmented/picker) | `sys.remote_scpi`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Kept |
| menus-452 | `RemVxi`: VXI-11 | toggle (segmented/picker) | `sys.remote_vxi11`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Kept |
| menus-1253 | `RemTelnet`: telnet | toggle | `sys.remote_telnet`; Off, On; sys_save; listener follows desired state | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Added |
| menus-453 | `RemSsh`: SSH | toggle (segmented/picker) | `sys.remote_ssh`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Kept |
| menus-454 | `RemMdns`: mDNS | toggle (segmented/picker) | `sys.remote_mdns`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Kept |
| menus-455 | `NetWeb`: Address | status (text only) | `Ui.net; sys.remote_web`; read-only web address, off, or not connected | Always | text only | keep | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Kept |
| menus-456 | `RemScpiInfo`: Port | status (text only) | `sys.remote_scpi`; read-only off or 5555 (TCP) | Always | text only | keep | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Kept |
| menus-457 | `RemVxiInfo`: Port | status (text only) | `sys.remote_vxi11`; read-only off or 111 and CORE_PORT (LXI) | Always | text only | keep | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Changed |
| menus-1254 | `RemTelnetInfo`: Port | status | remote telnet port and configured state; no action | Always | text only | keep | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Added |
| menus-458 | `Open:Ssh`: SSH settings | submenu | `open_menu(Ssh); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/ssh.rs`; `ui/src/ui/menu/ssh.rs` | Kept |
| menus-459 | `RemName`: Name | picker | `sys.net_name; net_name/name_value`; default name, current custom name, Other keyboard; 1..63 lowercase letters/digits/hyphen; D-Name | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/remote.rs`; `ui/src/ui/menu/remote.rs` | Kept |
| menus-1909 | `RemAllow`: Allowed | picker | `sys.net_allow`; the list in force, the networks the scope is on, Other keyboard (`netacl::canonical`; a bad word refuses the whole entry, empty removes the line); written and put in force at once (servers' list, firewall and SSH off the frame loop) | Always | S/P by committed index; collapsed value filled; focus outline independent | committed-choice highlight (task 81) | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs`; `ui/src/ui/active_choice/system.rs` | Added (upstream c24899c4, 256a48d4) |
| menus-1910 | `RemFirewall`: Firewall | status (text only) | `sys.net_allow`, last run; on, off: everyone, applying, failed: see the log | Always | text only | keep | `ui/src/ui/menu/remote.rs`; `ui/src/ui/remote.rs` | Added (upstream c24899c4) |

### Ssh

Path: **Network > Remote services > SSH settings**. Menu requirement: **menus-36**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/ssh.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-460 | `SshState`: State | status (text only) | `Ui.ssh.state; sys.remote_ssh`; read-only off/not started/service state | Always | text only | keep | `ui/src/ui/menu/ssh.rs`; `ui/src/ui/ssh.rs` | Kept |
| menus-461 | `SshPassword`: Password login | toggle (segmented/picker) | `sys.ssh_password; ssh_adjust`; Off, On; Off rejected without trusted key | always; cannot turn off without trusted key | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/ssh.rs`; `ui/src/ui/ssh.rs` | Kept |
| menus-462 | `SshShowPass`: Show password | action | `ssh_password_lines(); Ui.dialog`; show existing password dialog; no password changes | Always | focus border only | keep | `ui/src/ui/menu/ssh.rs`; `ui/src/ui/ssh.rs` | Kept |
| menus-463 | `SshHostKey`: Host key | status (text only) | `Ui.ssh.host_fp`; read-only fingerprint preview or not made yet | Always | text only | keep | `ui/src/ui/menu/ssh.rs`; `ui/src/ui/ssh.rs` | Kept |
| menus-464 | `SshKeys`: Trusted keys | status (text only) | `Ui.ssh.keys`; read-only trusted-key count or none | Always | text only | keep | `ui/src/ui/menu/ssh.rs`; `ui/src/ui/ssh.rs` | Kept |
| menus-465 | `SshKeyList`: List keys | action | `ssh_key_lines(); Ui.dialog`; list trusted keys | Always | focus border only | keep | `ui/src/ui/menu/ssh.rs`; `ui/src/ui/ssh.rs` | Kept |
| menus-466 | `SshImport`: Import keys from USB | action | `ssh_import()`; import trusted public keys from USB; existing validation | Always; no-op without SSH directory; requires front USB stick | focus border only | keep | `ui/src/ui/menu/ssh.rs`; `ui/src/ui/ssh.rs` | Kept |
| menus-467 | `SshRemove`: Remove a key | action | `ssh::remove_allowed; open_picker(SshRemoveKey)`; refuse no keys or removal of last key with password login off | always; handler rejects last-key removal when password login off | focus border only | keep | `ui/src/ui/menu/ssh.rs`; `ui/src/ui/ssh.rs` | Kept |

### Time

Path: **System > Date and time**. Menu requirement: **menus-37**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/time.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-468 | `TimeNow`: Now | status (text only) | `host.unix_now; zone; clock_known`; read-only local date/time/zone, not set marker | Always | text only | keep | `ui/src/ui/menu/time.rs`; `ui/src/ui/time.rs` | Kept |
| menus-469 | `NtpOn`: Time from NTP | toggle (segmented/picker) | `sys.ntp`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/time.rs`; `ui/src/ui/time.rs` | Kept |
| menus-470 | `NtpServer`: NTP server | segmented choice / picker | `sys.ntp_server; set_server`; D-NTP; preset/current server + Other keyboard | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/time.rs`; `ui/src/ui/menu/time.rs` | Kept |
| menus-471 | `NtpStatus`: NTP state | status (text only) | `ntp_status()`; read-only NTP status/error | Always | text only | keep | `ui/src/ui/menu/time.rs`; `ui/src/ui/time.rs` | Kept |
| menus-472 | `NtpSync`: Sync now | action | `ntp_sync_now()`; request immediate time sync | Always; rejects NTP Off or no network link | focus border only | keep | `ui/src/ui/menu/time.rs`; `ui/src/ui/time.rs` | Kept |
| menus-473 | `TimeZone`: Time zone | segmented choice / picker | `sys.tz; set_tz`; DHCP + ZONES labels + custom POSIX zone + Other keyboard; D-Zone | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/time.rs`; `ui/src/ui/menu/time.rs` | Kept |
| menus-474 | `TimeSet`: Set the clock | action | `OskFor::DateTime`; date/time keyboard in current time zone | Always | focus border only | keep | `ui/src/ui/menu/time.rs`; `ui/src/ui/time.rs` | Kept |

### Bode

Path: **Menu > Analysers > Bode; active Bode overflow > Settings**. Menu requirement: **menus-38**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/bode.rs`.

Controlled generator: On, Run, Start, Stop, Points, Amp, In, Out, Gen, [Cur], [Link, Addr]. Free-running: On, Run, Start, Stop, SwTime, SwLog, Points, In, [Ref], Out, Gen, Sync, SyncEdge, SyncLvl, [Cur]. Brackets are conditional as listed in rows.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-475 | `BodeOn`: View | toggle (segmented/picker) | `m.view`; Off (Scope), On (Bode) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-1255 | `BodeMode`: Mode | segmented choice / picker | `m.bode_mode`; Loop, PSRR, Z; changing mode clears points/cursor | non-Free generator only; after View | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Added |
| menus-476 | `BodeRun`: Start / stop sweep | action | `bode_start / bode_finish`; start or stop sweep; validate generator connection and sources | Always | focus border only | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-477 | `BodeStart`: Start freq | numeric | `m.bode_start`; 1-2-5 ladder 1 Hz..5 GHz, <= stop/2 | Always | V | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-478 | `BodeStop`: Stop freq | numeric | `m.bode_stop`; 1-2-5 ladder 1 Hz..5 GHz, >= start*2 | Always | V | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-479 | `BodeSwTime`: Sweep time | numeric | `m.bode_sw_time`; 1-2-5 ladder 1 ms..500 s | free-running generator | V | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-480 | `BodeSwLog`: Sweep | toggle (segmented/picker) | `m.bode_sw_log`; Lin, Log | free-running generator | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-481 | `BodePoints`: Points | numeric | `m.bode_points`; 5..200; step 5 | Always | V | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-482 | `BodeAmp`: Amplitude | numeric | `m.bode_amp`; 1-2-5 ladder 1 Hz-style units, clamp 0.01..20 Vpp; legacy ladder begins at 1 | controlled generator | V | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-483 | `BodeIn`: Input (ref) | segmented choice / picker | `m.bode_in`; W-source; in Free-running sweep instead m.bode_sw_in = Off, CH; cycle sources; free source-on recipe preserved | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Changed |
| menus-484 | `BodeRef`: Ref level | numeric | `m.bode_ref`; -80..40 dBV; step 1 | free-running generator and bode_sw_in=0 | V | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-485 | `BodeOut`: Output | segmented choice / picker | `m.bode_out`; W-source; cycle sources; free source-on recipe preserved | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Changed |
| menus-486 | `BodeGen`: Generator | segmented choice / picker | `m.bode_gen`; D-Generators | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-487 | `BodeSync`: Sync | segmented choice / picker | `m.bode_sync`; CH | free-running generator | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-488 | `BodeSyncEdge`: Sync edge | toggle (segmented/picker) | `m.bode_sync_slope`; Rising, Falling | free-running generator | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-489 | `BodeSyncLvl`: Sync level | numeric | `m.bode_sync_level`; -20..20 V; step 0.1 | free-running generator | V | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-490 | `BodeCur`: Cursor | numeric | `Ui.bode.cursor`; 0..points.len()-1; step 1; display frequency/gain/phase | points not empty | V | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-491 | `BodeLink`: Connection | toggle (segmented/picker) | `m.bode_usb`; LAN, USB | external controlled generator | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-492 | `BodeAddr`: Gen address | numeric | `m.bode_addr`; IPv4 address[:port]; knob changes last octet 1..254; preserves port | external controlled generator and LAN connection | V | keep | `ui/src/ui/menu/bode.rs`; `ui/src/ui/bode.rs` | Kept |
| menus-493 | `SrcOn`: Switch on source | action | `off_sources(menu); m.ch[c].on`; switch on all missing analogue sources; reset focus | appended only when off_sources(menu) is nonempty; exact source set in recipe below | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |

### Afg

Path: **Menu > Instruments > Generator; GI key/GEN card**. Menu requirement: **menus-39**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/generator.rs`.

AfgSel only if gen_count()>1. Prefix: [Sel], On, Wave. DC adds Offset and returns. Arb adds Freq, Ampl, Offset, ArbSrc, ArbGrab, ArbFile and returns. Noise adds Ampl, Offset. Others add Freq, Ampl, Offset. Square/Pulse add Duty, Ramp adds Sym. Sine/Square/Ramp add Mod, then Rate/Depth/Shape if modulation is on.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-494 | `AfgSel`: Generator | segmented choice / picker | `m.gen_sel; gen_select`; Gen 1..gen_count() | gen_count()>1 | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-495 | `AfgOn`: Output | toggle (segmented/picker) | `m.afg_on`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-496 | `AfgWave`: Wave | segmented choice / picker | `m.afg_wave`; Sine, Square, Ramp, Pulse, Noise, DC, Arb | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-497 | `AfgFreq`: Frequency | numeric + picker | `m.afg_freq; afg_clamp`; 0.01 Hz..gen_max_hz(); 1-2-5 ladder plus cap; keypad units mHz/Hz/kHz/MHz | wave not DC or Noise | V; numeric keypad keys have focus only | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-498 | `AfgAmpl`: Amplitude | numeric | `m.afg_ampl; afg_clamp`; 0.002..10 Vpp ladder, further board().gen_clamp by frequency and offset | wave != DC | V | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-499 | `AfgOffset`: Offset | numeric | `m.afg_offset; afg_clamp`; V; step 0.1; DC -5..5 V; otherwise board().gen_clamp | Always | V | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-500 | `AfgArbSrc`: Capture from | segmented choice / picker | `m.afg_arb_src`; CH | wave=Arb | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-501 | `AfgArbGrab`: Capture the screen | action | `afg_arb_capture(afg_arb_src)`; capture waveform as arbitrary generator table; requires data | wave=Arb | focus border only | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-502 | `AfgArbFile`: Load arb.csv | action | `afg_arb_load; board().arb_parse`; load arb.csv; reject empty/invalid wave | wave=Arb | focus border only | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-503 | `AfgDuty`: Duty | numeric | `m.afg_duty`; 5..95%; step 5 | Square or Pulse | V | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-504 | `AfgSym`: Symmetry | numeric | `m.afg_symmetry`; 0..100%; step 5 | Ramp | V | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-505 | `AfgMod`: Modulation | segmented choice / picker | `m.afg_mod`; Off, AM, FM, PM | Sine/Square/Ramp | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-506 | `AfgModRate`: Mod. rate | numeric | `m.afg_mod_rate`; 0.01 Hz..1 MHz, 1-2-5 ladder (implementation lower bound) | Sine/Square/Ramp and modulation on | V | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-507 | `AfgModDepth`: Depth / dev. | numeric | `m.afg_mod_depth / afg_fm_dev / afg_pm_dev`; AM 0..120% step 10; FM 0.01 Hz..30 MHz ladder, carrier/cap constrained; PM 0..360 degrees step 15 | Sine/Square/Ramp and modulation on | V | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-508 | `AfgModShape`: Mod. shape | segmented choice / picker | `m.afg_mod_shape`; Sine, Square, Triangle, Up ramp, Down ramp, Noise | Sine/Square/Ramp and modulation on | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |

### BwTest

Path: **Menu > Analysers > Bandwidth; active bandwidth overflow > Settings**. Menu requirement: **menus-40**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/bandwidth.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-509 | `BwOn`: View | toggle (segmented/picker) | `m.view`; Off (Scope), On (Step) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bandwidth.rs`; `ui/src/ui/bwtest.rs` | Kept |
| menus-510 | `BwSrc`: Source | segmented choice / picker | `m.bw_src`; CH | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bandwidth.rs`; `ui/src/ui/bwtest.rs` | Kept |
| menus-511 | `BwHint`: Signal freq | segmented choice / picker | `m.bw_hint`; Auto, 1, 2, 5, 10, 20, 50, 100, 150, 200, 250, 300, 400, 500, 750 MHz, 1 GHz | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bandwidth.rs`; `ui/src/ui/bwtest.rs` | Kept |
| menus-512 | `BwBins`: Bins / period | segmented choice / picker | `m.bw_bins`; 250, 500, 1000, 2000, 5000 | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bandwidth.rs`; `ui/src/ui/bwtest.rs` | Kept |
| menus-513 | `SrcOn`: Switch on source | action | `off_sources(menu); m.ch[c].on`; switch on all missing analogue sources; reset focus | appended only when off_sources(menu) is nonempty; exact source set in recipe below | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Kept |

### ViewMore

Path: **active analyser or XY plot > three-dot button**. Menu requirement: **menus-41**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/view_more.rs`.

XY: Exit only. Eye: EyeClear, Settings, Exit. Spectrum: SpecPeak, Settings, Exit. Power/Bode/Step: Settings, Exit. Scope has no overflow button.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-514 | `EyeClear`: Clear eye | action | `Ui.azr.eye/eye_res/page`; clear eye accumulation, retain active view | overflow recipe above | focus border only | keep | `ui/src/ui/menu/eye.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-515 | `SpecPeak`: Centre on peak | action | `strongest_line(); m.spec_center/spec_span`; centre on strongest peak; initialize span if full | overflow recipe above | focus border only | keep | `ui/src/ui/menu/spectrum.rs`; `ui/src/ui/analysis.rs` | Kept |
| menus-516 | `ViewSettings`: Settings | action | `view_settings()`; open current analyser's settings menu; no XY settings target | overflow recipe above | focus border only | keep | `ui/src/ui/menu/view_more.rs` | Kept |
| menus-517 | `ViewExit`: Exit | action | `view_exit(); m.view/Ui.xy`; return to Scope; stop Bode sweep and release generator | overflow recipe above | focus border only | keep | `ui/src/ui/menu/view_more.rs` | Kept |

### La

Path: **Menu > Instruments > Logic analyser; LA key/card**. Menu requirement: **menus-42**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/logic.rs`.

LA port required for entry. Hardware probe check on entering while off; source code is the evidence, not a unit test performed for this task.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-518 | `LaOn`: LA | toggle (segmented/picker) | `m.la.on`; Off, On; available platform required | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-519 | `LaShow`: Lines | segmented choice / picker | `m.la.show`; D0-D7, D8-D15, All, Custom; Custom initially D0-D3 if not already custom | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-520 | `Open:LaLines`: Line select | submenu | `open_menu(LaLines); parent=current menu`; navigate; current summary required by H1 | Always | focus border only; no active summary | change H1: inline summary and selected-state cue | `ui/src/ui/la.rs`; `ui/src/ui/menu/logic.rs` | Kept |
| menus-521 | `LaHeight`: Lane height | segmented choice / picker | `m.la.height`; Small, Medium, Large; clamped step | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-522 | `LaThr:0`: Thr D0-7 | segmented choice / picker | `m.la.preset[0]`; D-LAThreshold; p=0 then 1 | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-523 | `LaUser:0`: User D0-7 | numeric | `m.la.user[0]`; -15..15 V; step 0.01 V | pod preset=User | V | keep | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-524 | `LaThr:1`: Thr D8-15 | segmented choice / picker | `m.la.preset[1]`; D-LAThreshold; p=0 then 1 | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-525 | `LaUser:1`: User D8-15 | numeric | `m.la.user[1]`; -15..15 V; step 0.01 V | pod preset=User | V | keep | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |

### LaLines

Path: **Logic analyser > Line select**. Menu requirement: **menus-43**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/logic.rs`.

D0..D15 in numeric order, independent of whether the lane is currently shown.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-526 | `LaLine:0`: D0 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-527 | `LaLine:1`: D1 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-528 | `LaLine:2`: D2 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-529 | `LaLine:3`: D3 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-530 | `LaLine:4`: D4 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-531 | `LaLine:5`: D5 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-532 | `LaLine:6`: D6 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-533 | `LaLine:7`: D7 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-534 | `LaLine:8`: D8 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-535 | `LaLine:9`: D9 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-536 | `LaLine:10`: D10 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-537 | `LaLine:11`: D11 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-538 | `LaLine:12`: D12 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-539 | `LaLine:13`: D13 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-540 | `LaLine:14`: D14 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-541 | `LaLine:15`: D15 | toggle (segmented/picker) | `m.la.show bit n`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |

### LaPat

Path: **Trigger > D0-D15 (Pattern/Duration with LA on)**. Menu requirement: **menus-44**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/logic.rs`.

D0..D15 in numeric order; per-line H/L/X.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-542 | `LaPat:0`: D0 | segmented choice / picker | `m.pat_la[0]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-543 | `LaPat:1`: D1 | segmented choice / picker | `m.pat_la[1]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-544 | `LaPat:2`: D2 | segmented choice / picker | `m.pat_la[2]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-545 | `LaPat:3`: D3 | segmented choice / picker | `m.pat_la[3]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-546 | `LaPat:4`: D4 | segmented choice / picker | `m.pat_la[4]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-547 | `LaPat:5`: D5 | segmented choice / picker | `m.pat_la[5]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-548 | `LaPat:6`: D6 | segmented choice / picker | `m.pat_la[6]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-549 | `LaPat:7`: D7 | segmented choice / picker | `m.pat_la[7]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-550 | `LaPat:8`: D8 | segmented choice / picker | `m.pat_la[8]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-551 | `LaPat:9`: D9 | segmented choice / picker | `m.pat_la[9]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-552 | `LaPat:10`: D10 | segmented choice / picker | `m.pat_la[10]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-553 | `LaPat:11`: D11 | segmented choice / picker | `m.pat_la[11]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-554 | `LaPat:12`: D12 | segmented choice / picker | `m.pat_la[12]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-555 | `LaPat:13`: D13 | segmented choice / picker | `m.pat_la[13]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-556 | `LaPat:14`: D14 | segmented choice / picker | `m.pat_la[14]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |
| menus-557 | `LaPat:15`: D15 | segmented choice / picker | `m.pat_la[15]`; H, L, X | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logic.rs`; `ui/src/ui/la.rs` | Kept |

### Windows

Path: **Display > Windows**. Menu requirement: **menus-45**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Changed**. Constructor: `ui/src/ui/menu/windows.rs`.

If layout != One: append on analogue channels in channel order, active math channels M1..M8, then LA if on. Single-window layout has only Layout and Full screen.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-558 | `WinLayout`: Density | segmented choice / picker | `m.density; apply_density`; Scope, Full, Split, Four; updates full flag, pane and window layout. Split uses saved split_pane; multi-window density clears workspace Stacked. | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-559 | `FullScreen`: Full screen | toggle (segmented/picker) | `Ui.full`; Off, On; effective full screen suppressed while touch locked | Removed from this surface: FullScreen Item removed; Density Full and expand button replace it | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | e9f7c30b historical: `ui/src/ui/windows.rs`; `ui/src/ui/windows.rs`; current constructor `ui/src/ui.rs` | Dropped |
| menus-1256 | `Hdmi`: HDMI | segmented choice / picker | Same setting and value domain as menus-299; second row before Split pane selector | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs`; `ui/src/ui/windows.rs` | Added |
| menus-288 | `PaneSel`: Lower | segmented choice / picker | `m.pane; m.split_pane`; Traces, Spectrum, Results, Events, Trend, Bus view; Traces gives two stacked windows, another pane gives One | Windows menu after HDMI, only Density = Split; removed from Display | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/display.rs`; `ui/src/ui/windows.rs`; `ui/src/ui/tables.rs` | Changed |
| menus-560 | `WinCh:0`: CH1 | segmented choice / picker | `m.ch_win[0]`; 1..layout window count | m.win_layout != One, channel exists and is on | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-561 | `WinCh:1`: CH2 | segmented choice / picker | `m.ch_win[1]`; 1..layout window count | m.win_layout != One, channel exists and is on | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-562 | `WinCh:2`: CH3 | segmented choice / picker | `m.ch_win[2]`; 1..layout window count | m.win_layout != One, channel exists and is on | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-563 | `WinCh:3`: CH4 | segmented choice / picker | `m.ch_win[3]`; 1..layout window count | m.win_layout != One, channel exists and is on | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-1257 | `WinRef(i)`: R1..R10 | segmented choice / picker | `m.ref_win[i]`; 1..win_layout.count(), cycle; i=0..9 in slot order | win_layout != One and reference exists and is on; after CH and before M | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Added |
| menus-564 | `WinMath:0`: M1 | segmented choice / picker | `m.math_win[0]`; 1..layout window count | m.win_layout != One and math channel not Off | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-565 | `WinMath:1`: M2 | segmented choice / picker | `m.math_win[1]`; 1..layout window count | m.win_layout != One and math channel not Off | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-566 | `WinMath:2`: M3 | segmented choice / picker | `m.math_win[2]`; 1..layout window count | m.win_layout != One and math channel not Off | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-567 | `WinMath:3`: M4 | segmented choice / picker | `m.math_win[3]`; 1..layout window count | m.win_layout != One and math channel not Off | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-568 | `WinMath:4`: M5 | segmented choice / picker | `m.math_win[4]`; 1..layout window count | m.win_layout != One and math channel not Off | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-569 | `WinMath:5`: M6 | segmented choice / picker | `m.math_win[5]`; 1..layout window count | m.win_layout != One and math channel not Off | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-570 | `WinMath:6`: M7 | segmented choice / picker | `m.math_win[6]`; 1..layout window count | m.win_layout != One and math channel not Off | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-571 | `WinMath:7`: M8 | segmented choice / picker | `m.math_win[7]`; 1..layout window count | m.win_layout != One and math channel not Off | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |
| menus-572 | `WinLa`: LA | segmented choice / picker | `m.la_win`; 1..layout window count | m.win_layout != One and la_on() | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/windows.rs`; `ui/src/ui/windows.rs` | Changed |

### Logger

Path: **Menu > Instruments > Data logger**. Menu requirement: **menus-46**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/logger.rs`.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-573 | `LogOn`: Logging | toggle (segmented/picker) | `Ui.log.file; log_start/log_stop`; Off, On; active file determines state | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/logger.rs` | Kept |
| menus-574 | `LogIvl`: Interval | numeric | `m.log_ivl`; 1, 2, 5, 10, 30, 60, 300, 600, 1800, 3600 s; clamped | Always | V | keep | `ui/src/ui/menu/logger.rs` | Kept |
| menus-575 | `LogInfo`: File | status (text only) | `log_info()`; read-only file/status text | Always | text only | keep | `ui/src/ui/menu/logger.rs` | Kept |

### Pick

Path: **owning menu > choice value/label; dynamic picker**. Menu requirement: **menus-47**, keep; shared H1/N1 changes apply.
Delta vs e9f7c30b: **Kept**. Constructor: `ui/src/ui/menu/picker.rs`.

Only PickOpt(k) are rendered rows, in owning options order. The other identifiers are internal contexts, not extra visible rows. Ordinary list: select then return. Hex/frequency keypads edit a temporary buffer. WLAN/name/time/SSH helpers dispatch to their specialized picker handlers.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-576 | `PickOpt`:  | action / choice option | `Ui.pick; select_option / specialized picker dispatcher`; one row per option in owning item's order; keypads edit entry then commit | 0..options(owner).len()-1 | filled when option text equals value(owner); keypad keys unselected | change H1 for option lists; keep numeric keypad keys | `ui/src/ui/menu/picker.rs` | Kept |
| menus-577 | `AfgFreqEntry`: Enter frequency | picker context | `m.afg_freq; freq_key`; frequency keypad helper, FREQ_KEYS; see AfgFreq | PickOpt only rendered; others are context ids, not rows | context only; picker option rules | keep | `ui/src/ui/menu/generator.rs`; `ui/src/ui/afg.rs` | Kept |
| menus-578 | `WlanNet`: Networks | picker context | `Ui.wlan.scan; wlan_pick`; scanned networks in scan order + Other network; credentials keyboard if needed | PickOpt only rendered; others are context ids, not rows | context only; picker option rules | keep | `ui/src/ui/menu/network.rs` | Kept |
| menus-579 | `WlanForgetNet`: Forget | picker context | `wlan_forget_options; wlan_forget_pick`; saved network names, first 18, in saved-list order | PickOpt only rendered; others are context ids, not rows | context only; picker option rules | keep | `ui/src/ui/menu/network.rs` | Kept |
| menus-580 | `SshRemoveKey`: SshRemoveKey | picker context | `Ui.ssh.keys; ssh_remove_pick`; dynamic trusted-key labels; remove helper picker | PickOpt only rendered; others are context ids, not rows | context only; picker option rules | keep | `ui/src/ui/menu/ssh.rs`; `ui/src/ui/ssh.rs` | Kept |

### MeasHold

Path: **hold top Measure for 0.45 s**. Menu requirement: **menus-1258**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/measure.rs`. Runtime order: AddMeas only.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1259 | `AddMeas`: Add meas | action | `next_usual_meas / activate(MeasAdd)`; next absent Vpp, Freq, Period, Vrms, Vavg, Rise then ALL_MEAS; lift on Measure adds and closes; lift elsewhere keeps shortcut open | No next item: no-op; full selection rejects | focus border only | keep | `ui/src/ui/menu/measure.rs` | Added |

### MeasSelected

Path: **Measure catalogue > Selected**. Menu requirement: **menus-1260**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/measure.rs`. Runtime order: page of 16 remove rows, each standard row followed by Set; Clear all footer when nonempty; Prev/Next when >1 page.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1261 | `MeasRemove(index)`: Remove <source name> | action | `remove_measurement(index)` across singles, pairs then module rows; indices page*16..min(end,count); remove corresponding entry/override, clear stats/config | Configured row on current page | focus border only | keep | `ui/src/ui/menu/measure.rs`; `ui/src/ui/tools.rs` | Added |
| menus-1262 | `MeasCfg(index)`: Set | action | `open_meas_cfg(index)`; per-standard-measurement configuration below | Standard rows only, beside Remove; also added standard catalogue tiles | focus border only | keep | `ui/src/ui/menu/measure.rs`; `ui/src/ui/measurement_nav.rs` | Added |
| menus-1263 | `MeasClear`: Clear all | action | Same setting/action as menus-226 | measurement_count > 0 | focus border only | keep | `ui/src/ui/menu/measure.rs`; `ui/src/ui/measurement_nav.rs` | Added |
| menus-1264 | Prev | action | `Ui.meas_catalog_page`; saturating -1; no wrapping | selected_pages > 1 | focus border only | keep | `ui/src/ui/measurement_nav.rs` | Added |
| menus-1265 | Next | action | `Ui.meas_catalog_page`; clamped +1; no wrapping | selected_pages > 1 | focus border only | keep | `ui/src/ui/measurement_nav.rs` | Added |

### Bus

Path: **Main > Analysis > Bus**. Menu requirement: **menus-1266**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/bus.rs`. Runtime order: BusOn, BusLabel, BusPos, BusClk, BusClkThr, BusData, BusDataThr, BusRw.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1267 | `BusOn`: Bus | toggle | `m.bus.on`; Off, On; I2C trace only | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bus.rs`; `ui/src/ui/bus.rs` | Added |
| menus-1268 | `BusLabel`: Label | action | `m.bus.label`; bus label keyboard | Always | focus border only | keep | `ui/src/ui/menu/bus.rs`; `ui/src/ui/bus.rs` | Added |
| menus-1269 | `BusPos`: Position | numeric | `m.bus.pos`; -3.6..3.6 div, step .1 | Always | V | keep | `ui/src/ui/menu/bus.rs`; `ui/src/ui/bus.rs` | Added |
| menus-1270 | `BusClk`: Clock | segmented choice / picker | `m.bus.clk`; existing CH then all R1..R10, including empty slots; bus_ids | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bus.rs`; `ui/src/ui/bus.rs` | Added |
| menus-1271 | `BusClkThr`: Clock threshold | numeric | `m.bus.clk_thr`; -40..40 V; step source scale/25 (empty ref step .05 V) | Always | V | keep | `ui/src/ui/menu/bus.rs`; `ui/src/ui/bus.rs` | Added |
| menus-1272 | `BusData`: Data | segmented choice / picker | `m.bus.data`; existing CH then all R1..R10, including empty slots; bus_ids | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bus.rs`; `ui/src/ui/bus.rs` | Added |
| menus-1273 | `BusDataThr`: Data threshold | numeric | `m.bus.data_thr`; -40..40 V; step source scale/25 (empty ref step .05 V) | Always | V | keep | `ui/src/ui/menu/bus.rs`; `ui/src/ui/bus.rs` | Added |
| menus-1274 | `BusRw`: R/W bit | toggle | `m.bus.addr_rw`; Left out, Included | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/bus.rs`; `ui/src/ui/bus.rs` | Added |

### Magnetic

Path: **Main > Analysers > Magnetic**. Menu requirement: **menus-1275**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/magnetic.rs`. Runtime order: MagOn, MagV, MagI, MagN, MagAe, MagLe, MagRcu, MagPlot; MagCur if any nonempty loop; SrcOn if needed.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1276 | `MagOn`: View | toggle | `m.view`; Off(Scope), On(Magnetic) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/magnetic.rs`; `ui/src/ui/magnetic.rs` | Added |
| menus-1277 | `MagV`: Voltage | segmented choice / picker | `m.mag_v`; W-source | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/magnetic.rs`; `ui/src/ui/magnetic.rs` | Added |
| menus-1278 | `MagI`: Current | segmented choice / picker | `m.mag_i`; W-source | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/magnetic.rs`; `ui/src/ui/magnetic.rs` | Added |
| menus-1279 | `MagN`: Turns | numeric | `m.mag_n`; 1..10000, step 1 | Always | V | keep | `ui/src/ui/menu/magnetic.rs`; `ui/src/ui/magnetic.rs` | Added |
| menus-1280 | `MagAe`: Area | numeric | `m.mag_ae`; E(1e-6,.1,true) m²; 0 Off; emitted maximum .5 m² | Always | V | keep | `ui/src/ui/menu/magnetic.rs`; `ui/src/ui/magnetic.rs` | Added |
| menus-1281 | `MagLe`: Length | numeric | `m.mag_le`; E(1e-3,10,true) m; 0 Off; emitted maximum 50 m | Always | V | keep | `ui/src/ui/menu/magnetic.rs`; `ui/src/ui/magnetic.rs` | Added |
| menus-1282 | `MagRcu`: Rcu | numeric | `m.mag_rcu`; E(1e-3,1e3,true) Ω; 0 Ω; emitted maximum 5 kΩ | Always | V | keep | `ui/src/ui/menu/magnetic.rs`; `ui/src/ui/magnetic.rs` | Added |
| menus-1283 | `MagPlot`: Plot | segmented choice / picker | `m.mag_plot`; I-ψ; add B-H only if area and length positive | Always; value falls back to I-ψ without geometry | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/magnetic.rs`; `ui/src/ui/magnetic.rs` | Added |
| menus-1284 | `MagCur`: Cursor | numeric | `m.mag_cur`; last loop indices 0..len-1, step 1 | any result loop nonempty | V | keep | `ui/src/ui/menu/magnetic.rs`; `ui/src/ui/magnetic.rs` | Added |
| menus-1285 | `SrcOn`: Switch on source | action | Enable missing analogue mag_v/mag_i, same SrcOn handler; loaded refs do not need enable | off_sources nonempty | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Added |

### Inverter

Path: **Main > Analysers > Inverter**. Menu requirement: **menus-1286**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/inverter.rs`. Runtime order: conditional recipe below; SrcOn appended.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1287 | `InvOn`: View | toggle | `m.view`; Off(Scope), On(Inverter) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1288 | `InvPage`: Page | segmented choice / picker | `m.inv_page`; DQ0, Mechanical; Mechanical changes Off angle to Phase | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1289 | `InvA`: Phase A | segmented choice / picker | `m.inv_a`; W-source | DQ0 or Mechanical Phase angle | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1290 | `InvB`: Phase B | segmented choice / picker | `m.inv_b`; W-source | DQ0 or Mechanical Phase angle | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1291 | `InvC`: Phase C | segmented choice / picker | `m.inv_c`; W-source | DQ0 or Mechanical Phase angle | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1292 | `InvQty`: Signal | segmented choice / picker | `m.inv_volt`; Voltage, Current | DQ0 only | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1293 | `InvAng`: Angle | segmented choice / picker | `m.inv_ang`; DQ0: Off, Phase, Analog, Quad, Hall; Mechanical omits Off (legacy Off displays Phase) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1294 | `InvAn`: Analog | segmented choice / picker | `m.inv_an`; W-source | Analog angle | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1295 | `InvPoles`: Poles | numeric | `m.inv_poles`; 1..32, step 1 | DQ0: Off/Phase or Analog+Elec; Mechanical: Phase or Analog+Elec | V | keep | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1296 | `InvDeg`: Deg/V | numeric | `m.inv_degv`; E(1e-3,1e4,false) °/V; emitted maximum 50000 | Analog angle | V | keep | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1297 | `InvOff`: Offset | numeric | `m.inv_off`; -3600..3600°, step 1 | Analog angle | V | keep | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1298 | `InvElec`: Angle is | segmented choice / picker | `m.inv_elec`; Mech, Elec | Analog angle | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1299 | `InvQa`: A lead | source selector (numeric stepper) | `m.inv_qa`; D0..D15, clamped step 1 | Quad angle | V only; no committed-source fill | change H1: selected digital source cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1300 | `InvQb`: B | source selector (numeric stepper) | `m.inv_qb`; D0..D15, clamped step 1 | Quad angle | V only; no committed-source fill | change H1: selected digital source cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1301 | `InvQi`: Index | source selector (numeric stepper) | `m.inv_qi`; Off, D0..D15, clamped step 1 | Quad angle | V only; no committed-source fill | change H1: selected digital source cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1302 | `InvCpr`: Counts | numeric | `m.inv_cpr`; 1..100000, step 1 | Quad angle | V | keep | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1303 | `InvSwap`: Swap | toggle | `m.inv_swap`; Off, On | Quad angle | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1304 | `InvHa`: Hall A | source selector (numeric stepper) | `m.inv_ha`; D0..D15, clamped step 1 | Hall angle | V only; no committed-source fill | change H1: selected digital source cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1305 | `InvHb`: Hall B | source selector (numeric stepper) | `m.inv_hb`; D0..D15, clamped step 1 | Hall angle | V only; no committed-source fill | change H1: selected digital source cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1306 | `InvHc`: Hall C | source selector (numeric stepper) | `m.inv_hc`; D0..D15, clamped step 1 | Hall angle | V only; no committed-source fill | change H1: selected digital source cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1307 | `InvTq`: Torque | segmented choice / picker | `m.inv_tq`; Off then W-source | Mechanical only | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1308 | `InvScale`: Scale | numeric | `m.inv_scale`; E(1e-3,1e3,false); emitted maximum 5000 torque scale | Mechanical only | V | keep | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1309 | `InvEv`: Bus V | segmented choice / picker | `m.inv_ev`; Off then W-source | Mechanical only | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1310 | `InvEi`: Bus I | segmented choice / picker | `m.inv_ei`; Off then W-source | Mechanical only | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/inverter.rs`; `ui/src/ui/motor.rs` | Added |
| menus-1311 | `SrcOn`: Switch on source | action | Enable analogue source set from off_sources; DQ0 uses phases plus Analog if selected; Mechanical uses angle inputs plus optional torque/V/I | off_sources nonempty | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Added |

### Dpt

Path: **Main > Analysers > Double pulse**. Menu requirement: **menus-1312**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/dpt.rs`. Runtime order: DptOn, DptV, DptI, DptG, DptFourth, Dpt4 if enabled, DptPct, DptZoom; SrcOn if needed.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1313 | `DptOn`: View | toggle | `m.view`; Off(Scope), On(Dpt) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dpt.rs`; `ui/src/ui/dpt.rs` | Added |
| menus-1314 | `DptV`: Vds | segmented choice / picker | `m.dpt_v`; W-source | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dpt.rs`; `ui/src/ui/dpt.rs` | Added |
| menus-1315 | `DptI`: Id | segmented choice / picker | `m.dpt_i`; W-source | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dpt.rs`; `ui/src/ui/dpt.rs` | Added |
| menus-1316 | `DptG`: Vgs | segmented choice / picker | `m.dpt_g`; W-source | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dpt.rs`; `ui/src/ui/dpt.rs` | Added |
| menus-1317 | `DptFourth`: Fourth | segmented choice / picker | `m.dpt_4th`; Off, Gate, Diode V, Diode I | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dpt.rs`; `ui/src/ui/dpt.rs` | Added |
| menus-1318 | `Dpt4`: Channel | segmented choice / picker | `m.dpt_4ch`; W-source | Fourth != Off | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dpt.rs`; `ui/src/ui/dpt.rs` | Added |
| menus-1319 | `DptPct`: Gate | numeric | `m.dpt_pct`; 1..49%, step 1 | Always | V | keep | `ui/src/ui/menu/dpt.rs`; `ui/src/ui/dpt.rs` | Added |
| menus-1320 | `DptZoom`: Zoom | segmented choice / picker | `m.dpt_zoom`; Both, Turn-on, Turn-off; cyclic | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/dpt.rs`; `ui/src/ui/dpt.rs` | Added |
| menus-1321 | `SrcOn`: Switch on source | action | Enable missing analogue Vds/Id/Vgs plus fourth source if Fourth != Off | off_sources nonempty | focus border only | keep | `ui/src/ui/menu/navigation.rs` | Added |

### Features

Path: **Main > System > Features**. Menu requirement: **menus-1322**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/features.rs`. Runtime order: 8 analyser toggles in registry order, SIM, Load file, Open:Decoders(0).

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1323 | `FeatOn(0)`: Spectrum | toggle | `sys.features_off`; Off, On for registry feature spectrum; persist and enforce immediately; leaving out active analyser returns to Scope, spectrum pane closes, SIM playback stops | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1324 | `FeatOn(1)`: Eye | toggle | `sys.features_off`; Off, On for registry feature eye; persist and enforce immediately; leaving out active analyser returns to Scope, spectrum pane closes, SIM playback stops | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1325 | `FeatOn(2)`: Power | toggle | `sys.features_off`; Off, On for registry feature power; persist and enforce immediately; leaving out active analyser returns to Scope, spectrum pane closes, SIM playback stops | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1326 | `FeatOn(3)`: Bode | toggle | `sys.features_off`; Off, On for registry feature bode; persist and enforce immediately; leaving out active analyser returns to Scope, spectrum pane closes, SIM playback stops | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1327 | `FeatOn(4)`: Bandwidth test | toggle | `sys.features_off`; Off, On for feature bwtest; persist/enforce | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1328 | `FeatOn(5)`: Magnetic | toggle | `sys.features_off`; Off, On for registry feature magnetic; persist and enforce immediately; leaving out active analyser returns to Scope, spectrum pane closes, SIM playback stops | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1329 | `FeatOn(6)`: Inverter | toggle | `sys.features_off`; Off, On for registry feature inverter; persist and enforce immediately; leaving out active analyser returns to Scope, spectrum pane closes, SIM playback stops | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1330 | `FeatOn(7)`: Double pulse | toggle | `sys.features_off`; Off, On for registry feature dpt; persist and enforce immediately; leaving out active analyser returns to Scope, spectrum pane closes, SIM playback stops | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1331 | `FeatOn(8)`: SIM | toggle | `sys.features_off`; Off, On for registry feature sim; persist and enforce immediately; leaving out active analyser returns to Scope, spectrum pane closes, SIM playback stops | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1332 | `FeatOn(9)`: Load file | toggle | `sys.features_off`; Off, On for registry feature load; persist and enforce immediately; leaving out active analyser returns to Scope, spectrum pane closes, SIM playback stops | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1333 | `Open:Decoders(0)`: Decoders | submenu | Open first decoder availability page | Always | focus only | change H1: inline enabled/total count | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |

### Decoders(k)

Path: **System > Features > Decoders > More decoders**. Menu requirement: **menus-1334**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/features.rs`. Runtime order: FeatDec(i), i=k*18..min((k+1)*18,PROTOS.len()), then next-page link (last wraps to page 0).

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1335 | `FeatDec(i)`: protocol name | toggle | `sys.decoders_off`; Off, On; expands once per D-Protocols entry in registry order; persist/enforce. Refuses to turn off the last offered decoder | Current 18-entry page; compiled registry only; test Example only in test builds | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |
| menus-1336 | `Open:Decoders(next)`: More decoders / Decoders | submenu | Next page; after final page return to 0 (Decoders label); submenu history grows per hop | Always | focus only | change H1: inline page/enabled summary | `ui/src/ui/menu/features.rs`; `ui/src/ui/features.rs` | Added |

### Sim

Path: **Main > Analysis > SIM**. Menu requirement: **menus-1337**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/simulation.rs`. Runtime order: optional SimLive, Preset, Rate, Pattern, High, Low, Rise, BW, De-emph, Noise, Spread, Advanced, RJ, PJ, PJ freq, Slot, Play, Show, Defaults.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1338 | `SimLive`: Live | action | `return_to_live()`; restore held live setup; keep generated reference | record_mode only | focus border only | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1339 | `SimPreset`: Preset | segmented choice / picker | `m.sim_preset; apply_sim_preset`; D-SIM-presets in source order; resets preset knobs | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1340 | `SimRate`: Rate | numeric | `m.sim_rate`; E(1e8,64e9,false) bit/s; emitted maximum 50 Gbit/s | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1341 | `SimPat`: Pattern | segmented choice / picker | `m.sim_prbs`; PRBS-7,9,13,15,23,31 | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1342 | `SimHigh`: High | numeric | `m.sim_vhigh`; -2..2 V, step .05 | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1343 | `SimLow`: Low | numeric | `m.sim_vlow`; -2..2 V, step .05 | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1344 | `SimRise`: Rise 20-80% | numeric | `m.sim_rise`; E(5e-12,2e-9,false) s; emitted maximum 2.5 ns | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1345 | `SimBw`: Bandwidth | numeric | `m.sim_bw`; E(10e6,50e9,false) Hz; emitted maximum 50 GHz | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1346 | `SimDeemph`: De-emph | numeric | `m.sim_deemph`; 0..12 dB, step .5 | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1347 | `SimNoise`: Noise | numeric | `m.sim_noise`; E(100e-6,50e-3,false) V; emitted maximum 50 mV | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1348 | `SimSsc`: Spread | toggle | `m.sim_ssc`; Off, On | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1349 | `Open:SimAdvanced`: Advanced | submenu | Open SimAdvanced with current SIM context | Always | focus only; no summary | change H1: inline current SIM summary | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1350 | `SimRj`: RJ RMS | numeric | `m.sim_rj`; E(.1e-12,50e-12,false) s; emitted maximum 50 ps | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1351 | `SimPj`: PJ peak | numeric | `m.sim_pj`; E(.5e-12,50e-12,true) s; zero Off; emitted maximum 250 ps | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1352 | `SimPjHz`: PJ freq | numeric | `m.sim_pjhz`; E(1e3,50e6,false) Hz; emitted maximum 50 MHz; value says off at PJ=0 | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1353 | `SimSlot`: Slot | segmented choice / picker | `m.sim_slot`; R1..R10 | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1354 | `SimPlay`: Start/Run/Stop simulation; Cancel generation | action | `sim_activate`; start async generation, toggle record Run/Stop or cancel pending generation; dynamic label from item_label | Always; generation errors remain visible | focus border only | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1355 | `SimShow`: Show | toggle | `refs[sim_slot].on`; Off, On; empty slot displays empty and is no-op | Always | committed On/Off filled for populated slot; empty has no committed cue | keep committed identity and selected-value cue | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1356 | `SimDefaults`: Defaults | action | `apply_sim_preset`; regenerate if session/generation active, retaining requested Run/Stop intent | Always | focus border only | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |

### SimAdvanced

Path: **SIM > Advanced**. Menu requirement: **menus-1357**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/menu/simulation.rs`. Runtime order: Spread, SSC profile, SSC deviation, SSC frequency, DCD p-p, Corr RJ RMS, Corr RJ corner, Defaults.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1358 | `SimSsc`: Spread | toggle | `m.sim_ssc`; Off, On (same setting as SIM) | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1359 | `SimSscCenter`: SSC profile | segmented choice / picker | `m.sim_ssc_center`; Down, DC removed; Center | Always | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1360 | `SimSscDeviation`: SSC deviation | numeric | `m.sim_ssc_deviation`; 0..0.05, step .0005 (0..5%, .05 percentage-point step); pk for Center, p-p for Down | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1361 | `SimSscHz`: SSC frequency | numeric | `m.sim_ssc_hz`; E(1e3,1e6,false) Hz; emitted maximum 5 MHz | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1362 | `SimDcd`: DCD p-p | numeric | `m.sim_dcd_pp`; E(.5e-12,100e-12,true) s; zero Off; emitted maximum 250 ps | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1363 | `SimCorrRj`: Corr RJ RMS | numeric | `m.sim_corr_rj`; E(.1e-12,50e-12,true) s; zero Off; emitted maximum 50 ps | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1364 | `SimCorrHz`: Corr RJ corner | numeric | `m.sim_corr_hz`; E(1e3,100e6,false) Hz; emitted maximum 500 MHz | Always | V | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |
| menus-1365 | `SimDefaults`: Defaults | action | Same preset reset and regeneration as SIM Defaults | Always | focus border only | keep | `ui/src/ui/menu/simulation.rs`; `ui/src/ui/sim.rs` | Added |

### Catalogue controls

Path: **Measure catalogue**. Surface requirement: **menus-1366**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/meascatalog.rs`. Runtime order: Settings, Selected and Gate in header/side area, category tabs, source rows, group headers, tiles; order follows CATEGORIES and groups().

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1367 | Settings | submenu | `open_submenu(Measure)`; shared measurement settings | Always | focus only | change H1: inline gate/settings summary | `ui/src/ui/meascatalog.rs` | Added |
| menus-1368 | Selected (count) | submenu | `open_submenu(MeasSelected); meas_catalog_page=0`; exact total | Always | focus only | change H1: inline active count cue | `ui/src/ui/meascatalog.rs` | Added |
| menus-1369 | Gate | segmented choice | `m.meas_full_record`; Screen, Full record, exact tapped half | Always | active half filled | keep | `ui/src/ui/meascatalog.rs` | Added |
| menus-1370 | Category tab(k) | segmented choice | `Ui.cat_tab`; Basic, Two sources, Serial & PAM, Jitter & eye, Power, Motor drive, Power rails, Double pulse; changes tab/scroll, retains valid selected tile | k=0..7 in CATEGORIES order | current tab filled | keep | `ui/src/ui/meascatalog.rs` | Added |
| menus-1371 | Group header(k) | toggle | `Ui.cat_open[cat_tab]`; collapse current group or open exactly one; align opened group at list top | Groups of current category in CATEGORIES order; unlisted registered groups follow | chevron only; no active fill | change H1: selected expanded group cue | `ui/src/ui/meascatalog.rs` | Added |
| menus-1372 | Source A/B/C/D | picker | `Ui.meas_src/meas_src_b/cat_c/cat_d`; W-source; labels/number from CATEGORIES and max selected Def role; minimum 1/2/3 by category | 0..min(4,max(category.min,max_role+1)); A/B same identities as menus-219/220 | plain value and chevron | change H1: collapsed picker cue | `ui/src/ui/meascatalog.rs` | Added |
| menus-1373 | `CatSourceC`: Source C | picker | `Ui.cat_c`; W-source; own picker item for third/fourth source | When source_slots requests slot 2/3 | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs` | Added |
| menus-1374 | `CatSourceD`: Source D | picker | `Ui.cat_d`; W-source; own picker item for third/fourth source | When source_slots requests slot 2/3 | S/P by committed index; collapsed value filled; focus outline independent | keep committed identity and selected-value cue | `ui/src/ui/menu/measure.rs` | Added |

### Catalogue added measurements

Path: **Measure > category > group > tile**. Surface requirement: **menus-1375**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/measures/mod.rs`. Runtime order: CATEGORIES group order, then MODULES order, then each module defs order; repeated placements share id.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1376 | `CAT15`: Unit Interval | toggle (tile) | `m.cat`; (15, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue SERIAL > CLOCK & UNIT INTERVAL; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/serial.rs` | Added |
| menus-1377 | `CAT16`: NRZ Data Rate | toggle (tile) | `m.cat`; (16, A, B, C, D); reads roles A; toggle membership; output b/s; no numeric entry | Catalogue SERIAL > CLOCK & UNIT INTERVAL; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/serial.rs` | Added |
| menus-1378 | `MeasAdd:Median`: Median | toggle (tile) | `m.meas`; (Meas::Median, A); Def 207; toggle membership; output V; no numeric entry | Catalogue BASIC > AVERAGE & STATISTICS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1379 | `MeasAdd:Mode`: Mode | toggle (tile) | `m.meas`; (Meas::Mode, A); Def 208; toggle membership; output V; no numeric entry | Catalogue BASIC > AVERAGE & STATISTICS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1380 | `MeasAdd:LinSlope`: Linear Slope | toggle (tile) | `m.meas`; (Meas::LinSlope, A); Def 209; toggle membership; output V/s; no numeric entry | Catalogue BASIC > AVERAGE & STATISTICS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1381 | `MeasAdd:LinInt`: Linear Intercept | toggle (tile) | `m.meas`; (Meas::LinInt, A); Def 210; toggle membership; output V; no numeric entry | Catalogue BASIC > AVERAGE & STATISTICS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1382 | `MeasAdd:HighTime`: High Time | toggle (tile) | `m.meas`; (Meas::HighTime, A); Def 33; toggle membership; output s; no numeric entry | Catalogue BASIC > TIMING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1383 | `MeasAdd:LowTime`: Low Time | toggle (tile) | `m.meas`; (Meas::LowTime, A); Def 34; toggle membership; output s; no numeric entry | Catalogue BASIC > TIMING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1384 | `MeasAdd:Burst`: Burst Width | toggle (tile) | `m.meas`; (Meas::Burst, A); Def 26; toggle membership; output s; no numeric entry | Catalogue BASIC > TIMING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1385 | `MeasAdd:Outside`: Time Outside Level | toggle (tile) | `m.meas`; (Meas::Outside, A); Def 29; toggle membership; output s; no numeric entry | Catalogue BASIC > TIMING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1386 | `MeasAdd:NPeriods`: Duration N-Periods | toggle (tile) | `m.meas`; (Meas::NPeriods, A); Def 32; toggle membership; output s; no numeric entry | Catalogue BASIC > TIMING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1387 | `MeasAdd:CycAmp`: Cycle Amplitude | toggle (tile) | `m.meas`; (Meas::CycAmp, A); Def 94; toggle membership; output V; no numeric entry | Catalogue BASIC > PER CYCLE; POWER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1388 | `MeasAdd:CycTop`: Cycle Top | toggle (tile) | `m.meas`; (Meas::CycTop, A); Def 95; toggle membership; output V; no numeric entry | Catalogue BASIC > PER CYCLE; POWER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1389 | `MeasAdd:CycBase`: Cycle Base | toggle (tile) | `m.meas`; (Meas::CycBase, A); Def 96; toggle membership; output V; no numeric entry | Catalogue BASIC > PER CYCLE; POWER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1390 | `MeasAdd:CycPp`: Cycle Peak-to-Peak | toggle (tile) | `m.meas`; (Meas::CycPp, A); Def 97; toggle membership; output V; no numeric entry | Catalogue BASIC > PER CYCLE; POWER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1391 | `MeasAdd:CycMax`: Cycle Maximum | toggle (tile) | `m.meas`; (Meas::CycMax, A); Def 98; toggle membership; output V; no numeric entry | Catalogue BASIC > PER CYCLE; POWER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1392 | `MeasAdd:CycMin`: Cycle Minimum | toggle (tile) | `m.meas`; (Meas::CycMin, A); Def 99; toggle membership; output V; no numeric entry | Catalogue BASIC > PER CYCLE; POWER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1393 | `MeasAdd:EstUi`: Estimated UI | toggle (tile) | `m.meas`; (Meas::EstUi, A); Def 217; toggle membership; output s; no numeric entry | Catalogue SERIAL > CLOCK & UNIT INTERVAL; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1394 | `MeasAdd:PllUi`: PLL UI | toggle (tile) | `m.meas`; (Meas::PllUi, A); Def 218; toggle membership; output s; no numeric entry | Catalogue SERIAL > CLOCK & UNIT INTERVAL; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1395 | `MeasAdd:UiWtd`: Weighted UI | toggle (tile) | `m.meas`; (Meas::UiWtd, A); Def 219; toggle membership; output s; no numeric entry | Catalogue SERIAL > CLOCK & UNIT INTERVAL; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1396 | `MeasAdd:Bits`: Ones | toggle (tile) | `m.meas`; (Meas::Bits, A); Def 221; toggle membership; output count/ratio; no numeric entry | Catalogue SERIAL > CLOCK & UNIT INTERVAL; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1397 | `MeasAdd:Pam0`: PAM Level 0 | toggle (tile) | `m.meas`; (Meas::Pam0, A); Def 222; toggle membership; output V; no numeric entry | Catalogue SERIAL > PAM LEVELS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1398 | `MeasAdd:Pam1`: PAM Level 1 | toggle (tile) | `m.meas`; (Meas::Pam1, A); Def 223; toggle membership; output V; no numeric entry | Catalogue SERIAL > PAM LEVELS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1399 | `MeasAdd:Pam2`: PAM Level 2 | toggle (tile) | `m.meas`; (Meas::Pam2, A); Def 224; toggle membership; output V; no numeric entry | Catalogue SERIAL > PAM LEVELS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1400 | `MeasAdd:Pam3`: PAM Level 3 | toggle (tile) | `m.meas`; (Meas::Pam3, A); Def 225; toggle membership; output V; no numeric entry | Catalogue SERIAL > PAM LEVELS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1401 | `MeasAdd:PamSep`: PAM Level Separation | toggle (tile) | `m.meas`; (Meas::PamSep, A); Def 226; toggle membership; output V; no numeric entry | Catalogue SERIAL > PAM LEVELS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1402 | `MeasAdd:FoldRms`: Folded RMS | toggle (tile) | `m.meas`; (Meas::FoldRms, A); Def 220; toggle membership; output V; no numeric entry | Catalogue SERIAL > PAM LEVELS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/standard.rs` | Added |
| menus-1403 | `MeasPair:8`: Skew rise-rise | toggle (tile) | `m.meas2`; (pair 8, A, B); Def 19; also Pairs menu in index order; equal A/B rejected; toggle membership; output s; no numeric entry | Catalogue TWO > SKEW; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs` | Added |
| menus-1404 | `MeasPair:9`: Skew fall-fall | toggle (tile) | `m.meas2`; (pair 9, A, B); Def 236; also Pairs menu in index order; equal A/B rejected; toggle membership; output s; no numeric entry | Catalogue TWO > SKEW; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs` | Added |
| menus-1405 | `MeasPair:10`: Skew rise-fall | toggle (tile) | `m.meas2`; (pair 10, A, B); Def 237; also Pairs menu in index order; equal A/B rejected; toggle membership; output s; no numeric entry | Catalogue TWO > SKEW; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs` | Added |
| menus-1406 | `MeasPair:11`: Skew fall-rise | toggle (tile) | `m.meas2`; (pair 11, A, B); Def 238; also Pairs menu in index order; equal A/B rejected; toggle membership; output s; no numeric entry | Catalogue TWO > SKEW; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs` | Added |
| menus-1407 | `MeasPair:12`: Setup Time | toggle (tile) | `m.meas2`; (pair 12, A, B); Def 30; also Pairs menu in index order; equal A/B rejected; toggle membership; output s; no numeric entry | Catalogue TWO > SETUP & HOLD; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs` | Added |
| menus-1408 | `MeasPair:13`: Hold Time | toggle (tile) | `m.meas2`; (pair 13, A, B); Def 31; also Pairs menu in index order; equal A/B rejected; toggle membership; output s; no numeric entry | Catalogue TWO > SETUP & HOLD; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs` | Added |
| menus-1409 | `MeasPair:14`: Setup + Hold | toggle (tile) | `m.meas2`; (pair 14, A, B); Def 239; also Pairs menu in index order; equal A/B rejected; toggle membership; output s; no numeric entry | Catalogue TWO > SETUP & HOLD; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs` | Added |
| menus-1410 | `MeasPair:15`: Setup fall | toggle (tile) | `m.meas2`; (pair 15, A, B); Def 240; also Pairs menu in index order; equal A/B rejected; toggle membership; output s; no numeric entry | Catalogue TWO > SETUP & HOLD; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs` | Added |
| menus-1411 | `MeasPair:16`: Hold fall | toggle (tile) | `m.meas2`; (pair 16, A, B); Def 241; also Pairs menu in index order; equal A/B rejected; toggle membership; output s; no numeric entry | Catalogue TWO > SETUP & HOLD; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/ui/menu/measure.rs`; `ui/src/measures/pairs.rs` | Added |
| menus-1412 | `CAT40`: Jitter Summary | toggle (tile) | `m.cat`; (40, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1413 | `CAT41`: TIE | toggle (tile) | `m.cat`; (41, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1414 | `CAT42`: Phase Noise | toggle (tile) | `m.cat`; (42, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1415 | `CAT43`: TJ@BER | toggle (tile) | `m.cat`; (43, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1416 | `CAT44`: RJ-dd | toggle (tile) | `m.cat`; (44, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1417 | `CAT45`: DJ-dd | toggle (tile) | `m.cat`; (45, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1418 | `CAT46`: SJ@Freq | toggle (tile) | `m.cat`; (46, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1419 | `CAT47`: PJ | toggle (tile) | `m.cat`; (47, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1420 | `CAT48`: DDJ | toggle (tile) | `m.cat`; (48, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1421 | `CAT49`: DCD | toggle (tile) | `m.cat`; (49, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1422 | `CAT50`: SRJ | toggle (tile) | `m.cat`; (50, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1423 | `CAT51`: J2 | toggle (tile) | `m.cat`; (51, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > JITTER; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1424 | `CAT60`: Eye Height | toggle (tile) | `m.cat`; (60, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue JITTER > EYE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1425 | `CAT61`: Eye Width | toggle (tile) | `m.cat`; (61, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > EYE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1426 | `CAT62`: Eye High | toggle (tile) | `m.cat`; (62, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue JITTER > EYE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1427 | `CAT63`: Eye Height@BER | toggle (tile) | `m.cat`; (63, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue JITTER > EYE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1428 | `CAT64`: Eye Width@BER | toggle (tile) | `m.cat`; (64, A, B, C, D); reads roles A; toggle membership; output s; no numeric entry | Catalogue JITTER > EYE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1429 | `CAT65`: Eye Low | toggle (tile) | `m.cat`; (65, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue JITTER > EYE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1430 | `CAT66`: Q-Factor | toggle (tile) | `m.cat`; (66, A, B, C, D); reads roles A; toggle membership; output count/ratio; no numeric entry | Catalogue JITTER > EYE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1431 | `CAT70`: Bit High | toggle (tile) | `m.cat`; (70, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue JITTER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1432 | `CAT71`: Bit Low | toggle (tile) | `m.cat`; (71, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue JITTER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1433 | `CAT72`: Bit Amplitude | toggle (tile) | `m.cat`; (72, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue JITTER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1434 | `CAT73`: DC Common Mode | toggle (tile) | `m.cat`; (73, A, B, C, D); reads roles A,B; toggle membership; output V; no numeric entry | Catalogue JITTER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1435 | `CAT74`: AC Common Mode (Pk-Pk) | toggle (tile) | `m.cat`; (74, A, B, C, D); reads roles A,B; toggle membership; output V; no numeric entry | Catalogue JITTER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1436 | `CAT75`: Differential Crossover | toggle (tile) | `m.cat`; (75, A, B, C, D); reads roles A,B; toggle membership; output V; no numeric entry | Catalogue JITTER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1437 | `CAT76`: T/nT Ratio | toggle (tile) | `m.cat`; (76, A, B, C, D); reads roles A; toggle membership; output dB; no numeric entry | Catalogue JITTER > AMPLITUDE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1438 | `CAT80`: NRZ Clock Rate | toggle (tile) | `m.cat`; (80, A, B, C, D); reads roles A; toggle membership; output b/s; no numeric entry | Catalogue JITTER > TIMING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1439 | `CAT81`: SSC Freq Dev | toggle (tile) | `m.cat`; (81, A, B, C, D); reads roles A; toggle membership; output ppm; no numeric entry | Catalogue JITTER > TIMING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1440 | `CAT82`: SSC Modulation Rate | toggle (tile) | `m.cat`; (82, A, B, C, D); reads roles A; toggle membership; output Hz; no numeric entry | Catalogue JITTER > TIMING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/jitter.rs` | Added |
| menus-1441 | `CAT90`: Power Quality | toggle (tile) | `m.cat`; (90, A, B, C, D); reads roles A,B; toggle membership; output W; no numeric entry | Catalogue POWER > INPUT; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1442 | `CAT118`: Power Factor | toggle (tile) | `m.cat`; (118, A, B, C, D); reads roles A,B; toggle membership; output count/ratio; no numeric entry | Catalogue POWER > POWER FACTOR; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1443 | `CAT91`: Harmonics | toggle (tile) | `m.cat`; (91, A, B, C, D); reads roles B; toggle membership; output %; no numeric entry | Catalogue POWER > INPUT; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1444 | `CAT92`: Inrush Current | toggle (tile) | `m.cat`; (92, A, B, C, D); reads roles B; toggle membership; output A; no numeric entry | Catalogue POWER > INPUT; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1445 | `CAT93`: Input Capacitance | toggle (tile) | `m.cat`; (93, A, B, C, D); reads roles A,B; toggle membership; output F; no numeric entry | Catalogue POWER > INPUT; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1446 | `CAT100`: Turn On Time | toggle (tile) | `m.cat`; (100, A, B, C, D); reads roles A,B; toggle membership; output s; no numeric entry | Catalogue POWER > TIMING; RAILS > RAIL SEQUENCING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1447 | `CAT101`: Turn Off Time | toggle (tile) | `m.cat`; (101, A, B, C, D); reads roles A,B; toggle membership; output s; no numeric entry | Catalogue POWER > TIMING; RAILS > RAIL SEQUENCING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1448 | `CAT102`: E on | toggle (tile) | `m.cat`; (102, A, B, C, D); reads roles A,B; toggle membership; output J; no numeric entry | Catalogue POWER > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1449 | `CAT103`: E off | toggle (tile) | `m.cat`; (103, A, B, C, D); reads roles A,B; toggle membership; output J; no numeric entry | Catalogue POWER > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1450 | `CAT104`: E cond | toggle (tile) | `m.cat`; (104, A, B, C, D); reads roles A,B; toggle membership; output J; no numeric entry | Catalogue POWER > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1451 | `CAT105`: dv/dt rise | toggle (tile) | `m.cat`; (105, A, B, C, D); reads roles A,B; toggle membership; output V/s; no numeric entry | Catalogue POWER > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1452 | `CAT106`: dv/dt fall | toggle (tile) | `m.cat`; (106, A, B, C, D); reads roles A,B; toggle membership; output V/s; no numeric entry | Catalogue POWER > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1453 | `CAT107`: di/dt rise | toggle (tile) | `m.cat`; (107, A, B, C, D); reads roles A,B; toggle membership; output A/s; no numeric entry | Catalogue POWER > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1454 | `CAT108`: di/dt fall | toggle (tile) | `m.cat`; (108, A, B, C, D); reads roles A,B; toggle membership; output A/s; no numeric entry | Catalogue POWER > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1455 | `CAT109`: RDS(on) | toggle (tile) | `m.cat`; (109, A, B, C, D); reads roles A,B; toggle membership; output Ω; no numeric entry | Catalogue POWER > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1456 | `CAT113`: Ripple | toggle (tile) | `m.cat`; (113, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue POWER > OUTPUT; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1457 | `CAT114`: Line Ripple | toggle (tile) | `m.cat`; (114, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue POWER > OUTPUT; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1458 | `CAT115`: Switching Ripple | toggle (tile) | `m.cat`; (115, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue POWER > OUTPUT; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1459 | `CAT116`: PSRR | toggle (tile) | `m.cat`; (116, A, B, C, D); reads roles A,B; toggle membership; output dB; no numeric entry | Catalogue POWER > PSRR & IMPEDANCE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1460 | `CAT117`: Impedance | toggle (tile) | `m.cat`; (117, A, B, C, D); reads roles A,B; toggle membership; output count/ratio; no numeric entry | Catalogue POWER > PSRR & IMPEDANCE; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/power.rs` | Added |
| menus-1461 | `CAT150`: Overshoot | toggle (tile) | `m.cat`; (150, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue RAILS > RAIL SEQUENCING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/rails.rs` | Added |
| menus-1462 | `CAT151`: Undershoot | toggle (tile) | `m.cat`; (151, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue RAILS > RAIL SEQUENCING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/rails.rs` | Added |
| menus-1463 | `CAT152`: Turn-on Overshoot | toggle (tile) | `m.cat`; (152, A, B, C, D); reads roles A,B; toggle membership; output V; no numeric entry | Catalogue RAILS > RAIL SEQUENCING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/rails.rs` | Added |
| menus-1464 | `CAT153`: DC Rail | toggle (tile) | `m.cat`; (153, A, B, C, D); reads roles A; toggle membership; output V; no numeric entry | Catalogue RAILS > RAIL SEQUENCING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/rails.rs` | Added |
| menus-1465 | `CAT110`: Inductance | toggle (tile) | `m.cat`; (110, A, B, C, D); reads roles A,B; toggle membership; output H; no numeric entry | Catalogue POWER > MAGNETICS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/magnetic.rs` | Added |
| menus-1466 | `CAT111`: Magnetic Property | toggle (tile) | `m.cat`; (111, A, B, C, D); reads roles A,B; toggle membership; output count/ratio; no numeric entry | Catalogue POWER > MAGNETICS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/magnetic.rs` | Added |
| menus-1467 | `CAT112`: Magnetic Loss | toggle (tile) | `m.cat`; (112, A, B, C, D); reads roles A,B; toggle membership; output W; no numeric entry | Catalogue POWER > MAGNETICS; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/magnetic.rs` | Added |
| menus-1468 | `CAT130`: DQ0 | toggle (tile) | `m.cat`; (130, A, B, C, D); reads roles A,B,C; toggle membership; output count/ratio; no numeric entry | Catalogue MOTOR > DQ0 TRANSFORM; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/motor.rs` | Added |
| menus-1469 | `CAT131`: Speed | toggle (tile) | `m.cat`; (131, A, B, C, D); reads roles A,B,C; toggle membership; output rpm; no numeric entry | Catalogue MOTOR > SHAFT & EFFICIENCY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/motor.rs` | Added |
| menus-1470 | `CAT132`: Acceleration | toggle (tile) | `m.cat`; (132, A, B, C, D); reads roles A,B,C; toggle membership; output rpm/s; no numeric entry | Catalogue MOTOR > SHAFT & EFFICIENCY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/motor.rs` | Added |
| menus-1471 | `CAT133`: Direction | toggle (tile) | `m.cat`; (133, A, B, C, D); reads roles A,B,C; toggle membership; output count/ratio; no numeric entry | Catalogue MOTOR > SHAFT & EFFICIENCY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/motor.rs` | Added |
| menus-1472 | `CAT134`: Angle | toggle (tile) | `m.cat`; (134, A, B, C, D); reads roles A,B,C; toggle membership; output °; no numeric entry | Catalogue MOTOR > SHAFT & EFFICIENCY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/motor.rs` | Added |
| menus-1473 | `CAT135`: Torque | toggle (tile) | `m.cat`; (135, A, B, C, D); reads roles A,B,C,D; toggle membership; output N·m; no numeric entry | Catalogue MOTOR > SHAFT & EFFICIENCY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/motor.rs` | Added |
| menus-1474 | `CAT136`: Mechanical Power | toggle (tile) | `m.cat`; (136, A, B, C, D); reads roles A,B,C,D; toggle membership; output W; no numeric entry | Catalogue MOTOR > SHAFT & EFFICIENCY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/motor.rs` | Added |
| menus-1475 | `CAT137`: System Efficiency | toggle (tile) | `m.cat`; (137, A, B, C, D); reads roles A,B,C,D; toggle membership; output count/ratio; no numeric entry | Catalogue MOTOR > SHAFT & EFFICIENCY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/motor.rs` | Added |
| menus-1476 | `CAT170`: Vpeak | toggle (tile) | `m.cat`; (170, A, B, C, D); reads roles A,B,C; toggle membership; output V; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1477 | `CAT171`: Ipeak | toggle (tile) | `m.cat`; (171, A, B, C, D); reads roles A,B,C; toggle membership; output A; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1478 | `CAT172`: Td(on) | toggle (tile) | `m.cat`; (172, A, B, C, D); reads roles A,B,C; toggle membership; output s; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1479 | `CAT175`: Td(off) | toggle (tile) | `m.cat`; (175, A, B, C, D); reads roles A,B,C; toggle membership; output s; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1480 | `CAT173`: Tr | toggle (tile) | `m.cat`; (173, A, B, C, D); reads roles A,B,C; toggle membership; output s; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1481 | `CAT176`: Tf | toggle (tile) | `m.cat`; (176, A, B, C, D); reads roles A,B,C; toggle membership; output s; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1482 | `CAT174`: Ton | toggle (tile) | `m.cat`; (174, A, B, C, D); reads roles A,B,C; toggle membership; output s; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1483 | `CAT177`: Toff | toggle (tile) | `m.cat`; (177, A, B, C, D); reads roles A,B,C; toggle membership; output s; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1484 | `CAT178`: dv/dt | toggle (tile) | `m.cat`; (178, A, B, C, D); reads roles A,B,C; toggle membership; output V/s; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1485 | `CAT179`: di/dt | toggle (tile) | `m.cat`; (179, A, B, C, D); reads roles A,B,C; toggle membership; output A/s; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1486 | `CAT180`: Tdt | toggle (tile) | `m.cat`; (180, A, B, C, D); reads roles A,B,C; toggle membership; output s; no numeric entry | Catalogue DPT > SWITCHING; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1487 | `CAT181`: Trr | toggle (tile) | `m.cat`; (181, A, B, C, D); reads roles A,B,C; toggle membership; output s; no numeric entry | Catalogue DPT > DIODE RECOVERY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1488 | `CAT182`: Qrr | toggle (tile) | `m.cat`; (182, A, B, C, D); reads roles A,B,C; toggle membership; output C; no numeric entry | Catalogue DPT > DIODE RECOVERY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1489 | `CAT183`: Err | toggle (tile) | `m.cat`; (183, A, B, C, D); reads roles A,B,C; toggle membership; output J; no numeric entry | Catalogue DPT > DIODE RECOVERY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1490 | `CAT184`: Irrm | toggle (tile) | `m.cat`; (184, A, B, C, D); reads roles A,B,C; toggle membership; output A; no numeric entry | Catalogue DPT > DIODE RECOVERY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1491 | `CAT185`: Diode d/dt | toggle (tile) | `m.cat`; (185, A, B, C, D); reads roles A,B,C; toggle membership; output A/s; no numeric entry | Catalogue DPT > DIODE RECOVERY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |
| menus-1492 | `CAT186`: Qoss | toggle (tile) | `m.cat`; (186, A, B, C, D); reads roles A,B,C; toggle membership; output C; no numeric entry | Catalogue DPT > DIODE RECOVERY; capacity rejection shared; missing data gives unavailable result | filled when added; last selected tile outlined | keep | `ui/src/measures/double_pulse.rs` | Added |

### Decoder registry

Path: **Decode > Protocol; Trigger > Bus > Protocol; Features > Decoders**. Surface requirement: **menus-1493**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/protocols/mod.rs`. Runtime order: MODULES declaration order, filtered by Cargo feature and sys.decoders_off (Features pages retain off protocols).

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1494 | UART | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [Baud, Bits, Parity, Fmt], then no own options; differential true; select set_dec_proto | Core always built | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/core.rs` | Added |
| menus-1495 | I2C | choice option | `m.dec_proto`; source roles [SDA, SCL, -, -]; shared rows [none], then no own options; differential false; select set_dec_proto | Core always built | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/core.rs` | Added |
| menus-1496 | SPI | choice option | `m.dec_proto`; source roles [MOSI, Clock, CS, -]; shared rows [SpiMode, Bits], then no own options; differential false; select set_dec_proto | Core always built | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/core.rs` | Added |
| menus-1497 | CAN | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [CanRate, CanH], then no own options; differential true; select set_dec_proto | Core always built | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/core.rs` | Added |
| menus-1498 | LIN | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [Baud], then no own options; differential true; select set_dec_proto | Core always built | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/core.rs` | Added |
| menus-1499 | Parallel | choice option | `m.dec_proto`; source roles [D0, -, Clock, -]; shared rows [Bits], then no own options; differential false; select set_dec_proto | Core always built | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/core.rs` | Added |
| menus-1500 | C-PHY | choice option | `m.dec_proto`; source roles [A, B, C, -]; shared rows [CphyRate], then no own options; differential false; select set_dec_proto | Core always built | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/core.rs` | Added |
| menus-1501 | DMX512 | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then no own options; differential true; select set_dec_proto | Cargo dec-dmx512 and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/dmx512.rs` | Added |
| menus-1502 | S/PDIF | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then no own options; differential false; select set_dec_proto | Cargo dec-spdif and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/spdif.rs` | Added |
| menus-1503 | USB PD | choice option | `m.dec_proto`; source roles [CC, -, -, -]; shared rows [none], then no own options; differential false; select set_dec_proto | Cargo dec-usbpd and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/usbpd.rs` | Added |
| menus-1504 | DCC | choice option | `m.dec_proto`; source roles [Rail, -, -, -]; shared rows [none], then no own options; differential true; select set_dec_proto | Cargo dec-dcc and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/dcc.rs` | Added |
| menus-1505 | Manchester | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then DecOpt(0) Baud; differential false; select set_dec_proto | Cargo dec-linecode and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/linecode.rs` | Added |
| menus-1506 | Manchester `DecOpt(0)`: Baud | segmented choice / picker | `m.dec_opt[0]`; 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 460800, 921600; option default index 7; cyclic | Selected protocol Manchester | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/linecode.rs` | Added |
| menus-1507 | MLT-3 | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then DecOpt(0) Baud; differential false; select set_dec_proto | Cargo dec-linecode and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/linecode.rs` | Added |
| menus-1508 | MLT-3 `DecOpt(0)`: Baud | segmented choice / picker | `m.dec_opt[0]`; 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 460800, 921600; option default index 7; cyclic | Selected protocol MLT-3 | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/linecode.rs` | Added |
| menus-1509 | UART 9-bit | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then DecOpt(0) Baud; differential true; select set_dec_proto | Cargo dec-serial and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/serial.rs` | Added |
| menus-1510 | UART 9-bit `DecOpt(0)`: Baud | segmented choice / picker | `m.dec_opt[0]`; 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 460800, 921600; option default index 3; cyclic | Selected protocol UART 9-bit | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/serial.rs` | Added |
| menus-1511 | Modbus RTU | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then DecOpt(0) Baud, DecOpt(1) Parity; differential true; select set_dec_proto | Cargo dec-serial and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/serial.rs` | Added |
| menus-1512 | Modbus RTU `DecOpt(0)`: Baud | segmented choice / picker | `m.dec_opt[0]`; 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 460800, 921600; option default index 4; cyclic | Selected protocol Modbus RTU | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/serial.rs` | Added |
| menus-1513 | Modbus RTU `DecOpt(1)`: Parity | segmented choice / picker | `m.dec_opt[1]`; None, Even, Odd; option default index 1; cyclic | Selected protocol Modbus RTU | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/serial.rs` | Added |
| menus-1514 | Modbus ASCII | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then DecOpt(0) Baud, DecOpt(1) Data bits, DecOpt(2) Parity; differential true; select set_dec_proto | Cargo dec-serial and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/serial.rs` | Added |
| menus-1515 | Modbus ASCII `DecOpt(0)`: Baud | segmented choice / picker | `m.dec_opt[0]`; 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 460800, 921600; option default index 7; cyclic | Selected protocol Modbus ASCII | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/serial.rs` | Added |
| menus-1516 | Modbus ASCII `DecOpt(1)`: Data bits | segmented choice / picker | `m.dec_opt[1]`; 7, 8; option default index 0; cyclic | Selected protocol Modbus ASCII | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/serial.rs` | Added |
| menus-1517 | Modbus ASCII `DecOpt(2)`: Parity | segmented choice / picker | `m.dec_opt[2]`; Even, Odd; option default index 0; cyclic | Selected protocol Modbus ASCII | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/serial.rs` | Added |
| menus-1518 | 1-Wire | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then no own options; differential false; select set_dec_proto | Cargo dec-onewire and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/onewire.rs` | Added |
| menus-1519 | DS2482 | choice option | `m.dec_proto`; source roles [SDA, SCL, -, -]; shared rows [none], then no own options; differential false; select set_dec_proto | Cargo dec-onewire and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/onewire.rs` | Added |
| menus-1520 | SWD | choice option | `m.dec_proto`; source roles [SWDIO, SWCLK, -, -]; shared rows [none], then no own options; differential false; select set_dec_proto | Cargo dec-debug and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/debug.rs` | Added |
| menus-1521 | JTAG | choice option | `m.dec_proto`; source roles [TMS, TCK, TDI, TDO]; shared rows [none], then no own options; differential false; select set_dec_proto | Cargo dec-debug and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/debug.rs` | Added |
| menus-1522 | Quad-SPI | choice option | `m.dec_proto`; source roles [IO0, SCK, CS#, IO1]; shared rows [none], then no own options; differential false; select set_dec_proto | Cargo dec-qspi and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/qspi.rs` | Added |
| menus-1523 | I3C | choice option | `m.dec_proto`; source roles [SDA, SCL, -, -]; shared rows [none], then no own options; differential false; select set_dec_proto | Cargo dec-i3c and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/i3c.rs` | Added |
| menus-1524 | CAN-FD | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then DecOpt(0) Bit rate, DecOpt(1) Data rate, DecOpt(2) Signal; differential true; select set_dec_proto | Cargo dec-vehicle and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/vehicle.rs` | Added |
| menus-1525 | CAN-FD `DecOpt(0)`: Bit rate | segmented choice / picker | `m.dec_opt[0]`; 125 kbit/s, 250 kbit/s, 500 kbit/s, 800 kbit/s, 1 Mbit/s; option default index 2; cyclic | Selected protocol CAN-FD | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/vehicle.rs` | Added |
| menus-1526 | CAN-FD `DecOpt(1)`: Data rate | segmented choice / picker | `m.dec_opt[1]`; 1 M, 2 M, 4 M, 5 M, 8 M; option default index 1; cyclic | Selected protocol CAN-FD | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/vehicle.rs` | Added |
| menus-1527 | CAN-FD `DecOpt(2)`: Signal | segmented choice / picker | `m.dec_opt[2]`; RX/TX, CAN_H; option default index 0; cyclic | Selected protocol CAN-FD | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/vehicle.rs` | Added |
| menus-1528 | FlexRay | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then no own options; differential true; select set_dec_proto | Cargo dec-vehicle and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/vehicle.rs` | Added |
| menus-1529 | SENT | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then DecOpt(0) Clock tick, DecOpt(1) Data nibbles; differential false; select set_dec_proto | Cargo dec-vehicle and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/vehicle.rs` | Added |
| menus-1530 | SENT `DecOpt(0)`: Clock tick | segmented choice / picker | `m.dec_opt[0]`; 3 µs, 5 µs, 10 µs, 20 µs, 30 µs, 50 µs, 90 µs, Auto; option default index 0; cyclic | Selected protocol SENT | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/vehicle.rs` | Added |
| menus-1531 | SENT `DecOpt(1)`: Data nibbles | segmented choice / picker | `m.dec_opt[1]`; 1, 2, 3, 4, 5, 6; option default index 5; cyclic | Selected protocol SENT | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/vehicle.rs` | Added |
| menus-1532 | ARINC 429 | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then DecOpt(0) Bit rate; differential true; select set_dec_proto | Cargo dec-avionics and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/avionics.rs` | Added |
| menus-1533 | ARINC 429 `DecOpt(0)`: Bit rate | segmented choice / picker | `m.dec_opt[0]`; Auto, 12.5 k, 100 k; option default index 0; cyclic | Selected protocol ARINC 429 | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/ui/menu/decode.rs`; `ui/src/protocols/avionics.rs` | Added |
| menus-1534 | 1553 | choice option | `m.dec_proto`; source roles [Source, -, -, -]; shared rows [none], then no own options; differential false; select set_dec_proto | Cargo dec-avionics and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/avionics.rs` | Added |
| menus-1535 | I2S | choice option | `m.dec_proto`; source roles [SD, SCK, WS, -]; shared rows [Bits], then no own options; differential false; select set_dec_proto | Cargo dec-i2s and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/i2s.rs` | Added |
| menus-1536 | USB | choice option | `m.dec_proto`; source roles [D+, -, D-, -]; shared rows [UsbSpeed], then no own options; differential false; select set_dec_proto | Cargo dec-usb and offered by System > Features | committed option index filled; collapsed selector filled | keep committed identity and selected-value cue | `ui/src/protocols/usb.rs` | Added |

### Shared merged domains

Path: **all menus that reference these domains**. Surface requirement: **menus-1537**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/magnetic.rs`. Runtime order: as stated per domain.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1538 | W-source | domain / order recipe | CH1..CHn (including off channels), then R1..R10 only when slot has >=2 samples and dt>0; optional variant prepends Off. Source switching cycles this filtered order. | Always | inherited from referring row | keep | `ui/src/ui/magnetic.rs` | Added |
| menus-1539 | D-decoder-sources | domain / order recipe | CH1..CHn then all loaded R slots, including short loaded data; LA path uses D0..D15. Optional role adds None; C-PHY third role has no None. Trigger reference list also uses loaded slots; Bus deliberately offers every R slot, even empty. | Always | inherited from referring row | keep | `ui/src/ui/reference_decode.rs` | Added |
| menus-1540 | D-SIM-presets | domain / order recipe | PCIe 1.0, PCIe 2.0, PCIe 3.0, PCIe 4.0, PCIe 5.0, USB 2.0 HS, USB 3.2 Gen 1, USB 3.2 Gen 2, SATA I, SATA II, SATA III, 10GBASE-KR; values follow each Lane preset; choosing one resets SIM knobs, not the rest of the scope. | Always | inherited from referring row | keep | `dsp/src/serial.rs` | Added |
| menus-1541 | Engineering ladder | domain / order recipe | E(lo,hi,zero) exactly matches eng_ladder: optional 0; start e=lo, while e<=hi*1.001 and len<48 append e,2e,5e then e*=10. Anchor at first v with v+1e-12>=current, add signed d, clamp index. No final hi clamp: last entries can exceed hi, and non-decade lo yields a scaled 1-2-5 set. The absolute 1 ps anchor tolerance also affects sub-ps stepping. Keep this extraction distinct from a proposed correction. | Always | inherited from referring row | keep: value-set quirks do not conflict with H1/N1; separately ask owner before correcting bounds | `core/src/model/wave.rs` | Added |
| menus-1542 | Inverter order | domain / order recipe | DQ0: View,Page,A,B,C,Signal,Angle, Analog if chosen, Poles for Off/Phase or Analog+Elec, Deg/V,Offset,Angle is for Analog; Quad adds A lead,B,Index,Counts,Swap; Hall adds Hall A/B/C. Mechanical: View,Page,Angle; Phase adds A/B/C/Poles; Analog adds Analog,Deg/V,Offset,Angle is and Poles if Elec; Quad/Hall same groups; then Torque,Scale,Bus V,Bus I. | Always | inherited from referring row | keep | `ui/src/ui/motor.rs` | Added |

### Top and bottom bars

Path: **normal workspace chrome**. Surface requirement: **menus-1543**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui.rs`. Runtime order: top left-to-right and lower bars as described; these controls are outside MenuId item tables.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1544 | Run/status pill | action | `key(RUNSTOP)`; run/stop current acquisition or record owner | Top visible | status colour dot/fill (STOP/WAIT/TRIG/AUTO) | keep | `ui/src/ui.rs` | Added |
| menus-1545 | Single pill | action | `key(SINGLE)`; arm one acquisition/capture; same path as panel Single | Top visible | neutral button, no armed fill | keep | `ui/src/ui.rs` | Added |
| menus-1546 | Acquisition caption | submenu | Acquire while live, Reference while record_mode; rate/depth read from actual source or SIM/REF caption | Top visible | accent text while corresponding root open | change H1: inline source/setup summary | `ui/src/ui.rs` | Added |
| menus-1547 | View arrangement numbers | segmented choice | `view_go(i)`; i=0..views-1; switch stored arrangement, not analyser type | views >= 2 | current number filled | keep | `ui/src/ui/views.rs` | Added |
| menus-1548 | Live | action | `return_to_live()`; restore live setup retaining loaded records | record_mode; inserted before Measure | accent border/text | keep | `ui/src/ui.rs` | Added |
| menus-1549 | Measure | submenu | tap toggles catalogue MeasAdd; hold 0.45 s opens MeasHold | Top visible; label may shorten to Meas | border while root Measure; no setup summary | change H1: inline selected count | `ui/src/ui.rs` | Added |
| menus-1550 | Cursor | submenu | `toggle_menu(Cursor)`; top path preserves Off until explicit edit | Top visible | border while root Cursor | change H1: inline mode summary | `ui/src/ui.rs` | Added |
| menus-1551 | Display | submenu | `toggle_menu(Display)`; label may shorten to Disp | Top visible | border while Display open | change H1: inline density/format summary | `ui/src/ui.rs` | Added |
| menus-1552 | Storage | submenu | `toggle_menu(Storage)`; label may shorten to Store | Top visible | border while Storage open | change H1: inline destination summary | `ui/src/ui.rs` | Added |
| menus-1553 | LAN / WLAN / USB boxes | submenu | Network for LAN/WLAN; Storage for USB; state text/dot; same opener toggles | LAN always; WLAN stick / USB mounted respectively | border while destination root; connection dot | keep; H1 summary requirement applies | `ui/src/ui.rs` | Added |
| menus-1554 | Cards: Menu / Locked | submenu / action | open Main, or unlock touch without opening it | Cards lower bar; not Full | Main border; lock amber | keep | `ui/src/ui.rs` | Added |
| menus-1555 | Cards: CHn | submenu / toggle | select active channel; enable if off, toggle menu; hold toggles AC/DC | Existing channels; hidden in record_mode | channel colour identifies on/active/menu state | keep; H1 summary applies | `ui/src/ui.rs` | Added |
| menus-1556 | Cards: LA / GEN | submenu | LA off enables via la_key with probe guard; GEN only opens Afg | Hardware capabilities; hidden in record_mode | on/selected coloured border | keep; H1 summary applies | `ui/src/ui.rs` | Added |
| menus-1557 | Cards: TIME upper / lower | submenu / toggle | upper opens Horizontal; lower half toggles Zoom; unchanged s/div/delay readers | Cards bar | Horizontal border; zoom line accent | keep; H1 current Zoom cue applies | `ui/src/ui.rs` | Added |
| menus-1558 | Cards: Trigger source / sweep / body | choice / submenu | source cycles eligible LA/CH or record sources; sweep steps Auto/Normal/Single; body opens Trigger | Cards bar; preserves source context | source/sweep value text, root border | change H1: active-value cue | `ui/src/ui.rs` | Added |
| menus-1559 | Cards: stored Rn and on icon | submenu / toggle | select ref_sel/offline_ref/meas_src; body opens Reference; top-right icon toggles ref.on | record_mode; record_cards slots | record/source colour; On dot | keep; H1 on-state cue applies | `ui/src/ui.rs` | Added |
| menus-1560 | Compact: Menu / Unlock | submenu / action | Main or touch unlock | bar_compact; not Full | plain text/border | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1561 | Compact: Sources | submenu | open_sources popup | bar_compact; not Full | plain button | change H1: active source/count summary | `ui/src/ui/workspace.rs` | Added |
| menus-1562 | Compact: source pills | submenu / choice | CH then loaded on/selected R slots; select_workspace_source sets active/ref_sel and meas_src, then opens CH/Ref; does not enable a displayed CH | Available pill room; selected source swapped into last visible position if needed | active pill filled in source colour | keep; H1 inline menu summary applies | `ui/src/ui/workspace.rs` | Added |
| menus-1563 | Compact: +N | submenu | exact hidden source count; opens same Sources popup | Overflow when source pills exceed available count | plain text | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1564 | Compact: Overlay | segmented choice | `trace_mode(false)`; clears workspace.stacked, retains existing split windows if present | bar_compact | accent border/text when !workspace.stacked, no fill | change H1: active option fill | `ui/src/ui/workspace.rs` | Added |
| menus-1565 | Compact: Stacked | segmented choice | `trace_mode(true)`; one lane per visible source; Split/Four density returns to Scope | bar_compact | accent border/text while workspace.stacked, no fill | change H1: active option fill | `ui/src/ui/workspace.rs` | Added |
| menus-1566 | Compact: Time | submenu | Horizontal; read current detail/main time/div | bar_compact | plain value | change H1: inline Zoom state | `ui/src/ui/workspace.rs` | Added |
| menus-1567 | Compact: Trigger | submenu | Trigger; shows source/type/sweep/levels, whole rect opens menu | bar_compact | accent labels and root border | keep; H1 summary applies | `ui/src/ui/trigger_controls.rs` | Added |
| menus-1568 | Compact: Live | action | return_to_live; fallback when top Live absent | record_mode && no visible top Live | plain text | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1569 | Compact: Panel | toggle | `desktop_panel`; show/hide desktop front panel | Removed from this surface: the front panel belonged to the native Windows window, which moved to oos-desktop (ADR 0027) | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | historical: `ui/src/ui/workspace.rs` | Dropped |

### Sources popup

Path: **Compact > Sources or +N; plot context > Sources**. Surface requirement: **menus-1570**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/workspace.rs`. Runtime order: all existing CH, then loaded R slots, Logic analyser, Generator.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1571 | CHn | toggle | `workspace_action(Source(Ch(c)))`; Off/On; disables on CH, or activate_live_channel enables it and switches to live acquisition/Run; rejects removal of last trace | c=0..n_ch()-1 | On/Off text only | change H1: filled on-state | `ui/src/ui/workspace.rs` | Added |
| menus-1572 | Rn | toggle | `workspace_action(Source(Ref(i)))`; toggles ref.on, selects reference and measurement A; rejects removal of last trace | Loaded refs in slot order | On/Off text only | change H1: filled on-state | `ui/src/ui/workspace.rs` | Added |
| menus-1573 | Logic analyser | submenu | `key(LA)`; normal capability/probe checks and enable behaviour | Always appended, even without LA capability | text only | change H1: inline On/Off/unavailable | `ui/src/ui/workspace.rs` | Added |
| menus-1574 | Generator | submenu | `open_menu(Afg)`; no output enable | Always appended, even without generator capability | text only | change H1: inline output/unavailable | `ui/src/ui/workspace.rs` | Added |

### Plot context popup

Path: **right-click or stationary touch hold on plot**. Surface requirement: **menus-1575**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/workspace.rs`. Runtime order: Overlay, Stacked, Fit record, Reset zoom, Close zoom, Single acquisition, Run/Stop, Float measurements, trigger-source rows, Trigger settings, Sources.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1576 | Overlay traces | choice | Action::Overlay; same trace_mode(false) as footer | Always | check when !stacked | change H1: active option fill | `ui/src/ui/workspace.rs` | Added |
| menus-1577 | Stacked traces | choice | Action::Stacked; same trace_mode(true) as footer | Always | check when stacked | change H1: active option fill | `ui/src/ui/workspace.rs` | Added |
| menus-1578 | Fit record | action | `fit_selected_record()`; fit selected or fallback loaded record; closes zoom; no capture/Run change | Always; no data gives toast | none | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1579 | Reset zoom | action | clear box arm/path; reset horizontal detail to main and vertical transform to 1/0, keep Zoom on | Always; no-op without zoom | none | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1580 | Close zoom | action | clear box arm/path; set_zoom(false) | Always | none | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1581 | Single acquisition | action | key(SINGLE) | Always | none | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1582 | Run / Stop | action | key(RUNSTOP) | Always | none | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1583 | Float measurements | action | force statistics On, unfold, undock and place results card | Always | none | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1584 | Trigger on CHn / Rn | choice / action | set_trigger_source(id,false); switch acquisition owner preserving Run/Stop intent | first five trace_sources entries, omit Math; CH then R before M | none for selected source | change H1: highlight committed trigger source | `ui/src/ui/workspace.rs` | Added |
| menus-1585 | Trigger settings | submenu | open_menu(Trigger) | Always | none | change H1: inline type/source summary | `ui/src/ui/workspace.rs` | Added |
| menus-1586 | Sources | submenu | open_sources(56, bottom) | Always | none | change H1: inline active source/count | `ui/src/ui/workspace.rs` | Added |

### Eye and measurements context popups

Path: **right-click or stationary hold on Eye/results header**. Surface requirement: **menus-1587**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/workspace.rs`. Runtime order: Eye has six rows; measurements has five; first matching header determines popup.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1588 | Dock Eye right | choice / action | `workspace_action`; eye_float=None; eye_left=false; eye_place=Side | Eye header | no current placement mark | change H1: inline settings/active placement cue | `ui/src/ui/workspace.rs` | Added |
| menus-1589 | Dock Eye left | choice / action | `workspace_action`; eye_float=None; eye_left=true; eye_place=Side | Eye header | no current placement mark | change H1: inline settings/active placement cue | `ui/src/ui/workspace.rs` | Added |
| menus-1590 | Dock Eye below | choice / action | `workspace_action`; eye_float=None; eye_left=false; eye_place=Under | Eye header | no current placement mark | change H1: inline settings/active placement cue | `ui/src/ui/workspace.rs` | Added |
| menus-1591 | Float Eye | choice / action | `workspace_action`; eye_float default rectangle; relayout | Eye header | no current placement mark | change H1: inline settings/active placement cue | `ui/src/ui/workspace.rs` | Added |
| menus-1592 | Eye settings | submenu | `workspace_action`; open_menu(Eye) | Eye header | no current placement mark | change H1: inline settings/active placement cue | `ui/src/ui/workspace.rs` | Added |
| menus-1593 | Close Eye | action | `workspace_action`; view=Scope; eye_float=None; relayout | Eye header | no current placement mark | keep | `ui/src/ui/workspace.rs` | Added |
| menus-1594 | Float measurements | choice / action | `workspace_action`; force stats On, unfolded; measure_float default location; measure_docked=false; MeasureDock sets stats On and measure_docked=true | Measurements header | no current placement mark | change H1: selected placement/inline catalogue summary | `ui/src/ui/workspace.rs` | Added |
| menus-1595 | Dock right | choice / action | `workspace_action`; MeasureDock(Right); MeasureDock sets stats On and measure_docked=true | Measurements header | no current placement mark | change H1: selected placement/inline catalogue summary | `ui/src/ui/workspace.rs` | Added |
| menus-1596 | Dock below | choice / action | `workspace_action`; MeasureDock(Under); MeasureDock sets stats On and measure_docked=true | Measurements header | no current placement mark | change H1: selected placement/inline catalogue summary | `ui/src/ui/workspace.rs` | Added |
| menus-1597 | Dock left | choice / action | `workspace_action`; MeasureDock(Left); MeasureDock sets stats On and measure_docked=true | Measurements header | no current placement mark | change H1: selected placement/inline catalogue summary | `ui/src/ui/workspace.rs` | Added |
| menus-1598 | Add measurements | submenu | `workspace_action`; open_menu(MeasAdd); MeasureDock sets stats On and measure_docked=true | Measurements header | no current placement mark | change H1: selected placement/inline catalogue summary | `ui/src/ui/workspace.rs` | Added |

### Zoom and plot controls

Path: **plot expand/zoom corners and detail readout**. Surface requirement: **menus-1599**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui.rs`. Runtime order: expand, Zoom opener; when Zoom: H-,H+,V-,V+,Box,Reset,Close.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1600 | Expand | choice / action | toggle_expand: Scope/Full or Split/Four -> Full; effective Full suppressed by touch lock and separate monitor control mode | Plot corner | expand/restore icon | change H1: committed Density Full cue | `ui/src/ui.rs` | Added |
| menus-1601 | Zoom opener | toggle | set_zoom(!m.zoom); On stores main scale/fine/delay and defaults detail 3 time-scale steps finer, centres detail, vertical=1/0 | !view_on | icon/zoom state | keep; H1 cue applies | `ui/src/ui.rs` | Added |
| menus-1602 | H- | action | zoom_horizontal_step(+1), centre anchor; coarser zscale, clamped to main | Zoom && !view_on | none | keep | `ui/src/ui.rs` | Added |
| menus-1603 | H+ | action | zoom_horizontal_step(-1), centre anchor; finer zscale | Zoom && !view_on | none | keep | `ui/src/ui.rs` | Added |
| menus-1604 | V- | action | zoom_vertical_step(-1); factor /2 clamped 1..1024, centre anchor preserved | Zoom && !view_on | none | keep | `ui/src/ui.rs` | Added |
| menus-1605 | V+ | action | zoom_vertical_step(+1); factor *2 clamped 1..1024, centre anchor preserved | Zoom && !view_on | none | keep | `ui/src/ui.rs` | Added |
| menus-1606 | Box | toggle tool | arm_zoom_area toggles Ui.zoom_arm; disarms zone; repeated rectangles until plain tap or menu cancels | Zoom && !view_on; plot hold/wheel paths retain their own guards | filled while armed | keep | `ui/src/ui.rs` | Added |
| menus-1607 | Reset | action | reset_zoom; zscale=tscale,zdelay=delay,vertical=1/0; zoom remains on | Zoom | focus border only | keep | `ui/src/ui.rs` | Added |
| menus-1608 | Close | action | set_zoom(false) | Zoom and !XY and !view_on | accent text/border | keep | `ui/src/ui.rs` | Added |

### Per-measurement settings

Path: **catalogue added standard tile > Set; Selected > Set**. Surface requirement: **menus-1609**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/reflook.rs`. Runtime order: Source, Reference, then Edges used / Idle / Outside / N periods and Edges; X close.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1610 | Source -/+ | choice stepper | meas_cfg_source(-/+1); existing CH then loaded R; move the selected standard measurement and its override to new source; reject duplicate | Every standard row | value only | change H1: committed source cue | `ui/src/ui/reflook.rs` | Added |
| menus-1611 | Reference | toggle | MeasOwn.low/high override; toggle 10/90 and 20/80%; shared percentage inherited until edited | Every standard row | text only | change H1: active reference cue | `ui/src/ui/reflook.rs` | Added |
| menus-1612 | Edges used | status | qualified/total edges from analysis; Computing/Not retained/unavailable explicit | Rise or Fall | text only | keep | `ui/src/ui/reflook.rs` | Added |
| menus-1613 | Idle -/+ | numeric | MeasOwn.idle override; 0 period then 1-2-5 1 ns..5 s, clamped | Burst only | value only | keep | `ui/src/ui/reflook.rs` | Added |
| menus-1614 | Outside | choice | MeasOwn.side override; Above, Below, Both cyclic | Time Outside Level only | text only | change H1: active option cue | `ui/src/ui/reflook.rs` | Added |
| menus-1615 | N periods -/+ | numeric | MeasOwn.n override; 1..32, step1 | Duration N-Periods only | value only | keep | `ui/src/ui/reflook.rs` | Added |
| menus-1616 | Edges | toggle | MeasOwn.data override; Clock, Data | Duration N-Periods only | text only | change H1: active option cue | `ui/src/ui/reflook.rs` | Added |
| menus-1617 | X / outside tap | action | meas_cfg=None; retain committed edits | While config open | neutral X | keep | `ui/src/ui/reflook.rs` | Added |

### Load destination modal

Path: **Reference > Load file** in the native Windows window, and the `workflow load` command. Surface requirement:
**menus-1618**, tombstone: the modal only fed the native file dialog, which moved to oos-desktop (ADR 0027); drop,
retain the ids. Not rendered at HEAD.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1619 | Rn / Rn replace | action | slot=0..9; set load_destination, clear ask flag and open file request | Removed from this surface: the modal fed the native file dialog, which moved to oos-desktop (ADR 0027) | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | historical: `ui/src/ui/workflow.rs` | Dropped |
| menus-1620 | Cancel / outside / physical key | action | cancel_load_slot; no source or model change; file request cancelled | Removed from this surface: the modal fed the native file dialog, which moved to oos-desktop (ADR 0027) | Not rendered at HEAD | drop: superseded control; retain this id as a tombstone | historical: `ui/src/ui/workflow.rs` | Dropped |

### Eye header and auxiliary view controls

Path: **Eye header, its charts/column, waveform autoset bar and event/result panes**. Surface requirement: **menus-1621**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/eyepage.rs`. Runtime order: as listed, custom-rendered controls outside Item dispatch.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1622 | Eye source | picker | open_menu(Eye), open_picker(EyeSrc); W-source | Eye open; header | source colour/text, no selected-value fill | change H1: current source cue | `ui/src/ui/eyepage.rs` | Added |
| menus-1623 | Eye place | picker | open_menu(Eye), open_picker(EyePlace); Full/Side/Under | Eye header | current value only | change H1: placement cue | `ui/src/ui/eyepage.rs` | Added |
| menus-1624 | Eye settings / Close | submenu / action | settings opens Eye; Close returns Scope | Eye open | neutral header controls | change H1 for settings summary only | `ui/src/ui.rs` | Added |
| menus-1625 | Eye column tab and chart(k) | choice / toggle | eye_slide toggles side column; chart 0..3 expands in place, tapping enlarged chart restores four | !Full, eye column available/open | layout itself indicates selection | keep; H1 selected layout cue applies | `ui/src/ui/eyepage.rs` | Added |
| menus-1626 | Autoset Few / Many / Rise / Fall | action | auto_view: fit ~3/~10 periods, rising/falling edge ~2 divisions; source period required, else toast | autoset bar <30s old && !view_on | neutral icons, no persistent mode | keep | `ui/src/ui/autoset.rs` | Added |
| menus-1627 | Autoset bar X | action | auto_bar=None; outside tap also dismisses and continues | autoset bar visible | neutral X | keep | `ui/src/ui/autoset.rs` | Added |
| menus-1628 | Event table row / Close | action | row: go_to_time while stopped, or seg_show for segment; running rejects event navigation; Close clears table; scrollbar moves table_top | table open; rows in event/time order | row colours for validity, no selection | keep | `ui/src/ui/tables.rs` | Added |
| menus-1629 | Spectrum peak sort | choice / action | sort_toggle; alternate amplitude/frequency order | Full Spectrum with data or Spectrum pane | sort caption only | change H1: committed sort cue | `ui/src/ui/panes.rs` | Added |
| menus-1630 | Second pane title / Close | submenu / action | Spectrum title opens settings; Close pane/split_pane=0, Split becomes stacked trace windows | pane_h >0 | title/root border | change H1 summary only | `ui/src/ui/panes.rs` | Added |
| menus-1631 | Trend chart selector(k) | choice / action | `m.trend_sel[k]`; tapping a chart cycles directly to the next current measurement row ((row+1)%n)+2; no settings menu opens; no-op with no measurements | Trend pane; k in currently drawn charts | selected measurement name only | change H1: current trend cue | `ui/src/ui/panes.rs` | Added |
| menus-1632 | Result header / Prev / Next | toggle / action | header folds/unfolds results_min; page buttons saturate meas_page; same page survives docking | Results overlay/paged strip; arrows only when rows exceed page size | plain controls/page number | change H1: folded-state cue; keep paging | `ui/src/ui/measurement_nav.rs` | Added |

### Custom navigation contract

Path: **all custom catalogues, popups, modal and header controls**. Surface requirement: **menus-1633**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/ui/workspace.rs`. Runtime order: touch behaviour preserved; knob equivalents required by N1.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1634 | Popup and modal exit/input | navigation | Workspace popup clamps into plot, 30 px rows; tap inside executes and dismisses, outside dismisses/consumes. Right-click and stationary hold share context builder. Popup controls, catalogue tiles/categories, Selected page navigation, config X and slot modal are not all reachable through generic items/Multi today. Rebuild exposes their ordered actions, selected identities, Back/Close and cancel through the same menu model. Hold/drag distinguishes stationary hold from moving panels; moving after hold closes popup. | Touch lock blocks opening/edits; modal dialogs get input first | separate hover/focus from committed selection | change N1/H1: complete knob paths and 40 px targets | `ui/src/ui/workspace.rs` | Added |

### Catalogue category and group order

Path: **Measure > category**. Surface requirement: **menus-1635**, added; keep with H1/N1.
Delta vs e9f7c30b: **Added**. Constructor: `ui/src/measures/mod.rs`. Runtime order: category declaration order; groups ordered here; tiles use MODULES order then Def order.

| ID | Item / label | Kind | Field or action; value set / step | Visibility / enable conditions | Highlight today | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|---|---|---|
| menus-1636 | Basic | navigation / category | Groups: AMPLITUDE, AVERAGE & STATISTICS, TIMING, EDGES & COUNTS, PER CYCLE; source slots [Source A, Source B, Source C, Source D]; min 1; actual roles may add up to four | Selected category; groups may be folded | tab filled; group chevron; tiles filled by membership | keep; H1/N1 for custom groups/navigation | `ui/src/measures/mod.rs` | Added |
| menus-1637 | Two sources | navigation / category | Groups: DELAY, PHASE, SKEW, SETUP & HOLD; source slots [Source A, Source B, Source C, Source D]; min 2; actual roles may add up to four | Selected category; groups may be folded | tab filled; group chevron; tiles filled by membership | keep; H1/N1 for custom groups/navigation | `ui/src/measures/mod.rs` | Added |
| menus-1638 | Serial & PAM | navigation / category | Groups: CLOCK & UNIT INTERVAL, PAM LEVELS; source slots [Source A, Source B, Source C, Source D]; min 1; actual roles may add up to four | Selected category; groups may be folded | tab filled; group chevron; tiles filled by membership | keep; H1/N1 for custom groups/navigation | `ui/src/measures/mod.rs` | Added |
| menus-1639 | Jitter & eye | navigation / category | Groups: JITTER, EYE, AMPLITUDE, TIMING; source slots [Source A, Source B, Source C, Source D]; min 1; actual roles may add up to four | Selected category; groups may be folded | tab filled; group chevron; tiles filled by membership | keep; H1/N1 for custom groups/navigation | `ui/src/measures/mod.rs` | Added |
| menus-1640 | Power | navigation / category | Groups: INPUT, AMPLITUDE, TIMING, SWITCHING, MAGNETICS, OUTPUT, PSRR & IMPEDANCE, POWER FACTOR; source slots ["Voltage source", "Current source", "Source C", "Source D"]; min 2; actual roles may add up to four | Selected category; groups may be folded | tab filled; group chevron; tiles filled by membership | keep; H1/N1 for custom groups/navigation | `ui/src/measures/mod.rs` | Added |
| menus-1641 | Motor drive | navigation / category | Groups: DQ0 TRANSFORM, SHAFT & EFFICIENCY; source slots ["Phase A", "Phase B", "Phase C", "Torque"]; min 3; actual roles may add up to four | Selected category; groups may be folded | tab filled; group chevron; tiles filled by membership | keep; H1/N1 for custom groups/navigation | `ui/src/measures/mod.rs` | Added |
| menus-1642 | Power rails | navigation / category | Groups: RAIL SEQUENCING; source slots ["Rail", "Input", "Source C", "Source D"]; min 2; actual roles may add up to four | Selected category; groups may be folded | tab filled; group chevron; tiles filled by membership | keep; H1/N1 for custom groups/navigation | `ui/src/measures/mod.rs` | Added |
| menus-1643 | Double pulse | navigation / category | Groups: SWITCHING, DIODE RECOVERY; source slots ["Voltage", "Current", "Gate", "Source D"]; min 3; actual roles may add up to four | Selected category; groups may be folded | tab filled; group chevron; tiles filled by membership | keep; H1/N1 for custom groups/navigation | `ui/src/measures/mod.rs` | Added |

## Conditional order recipes

These recipes resolve the union tables above. Their fields and dispositions are the corresponding inventory rows;
the recipe rows preserve legacy ordering and conditions. `+` means concatenate, `[x if p]` means omit unless p.

| ID | Context | Exact order / condition | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|
| menus-1000 | Trigger prefix/suffix | Every type begins TrigType. After its body: TrigNoise if not Bus and no LA line; TrigAutoNoise, TrigMode, SearchOn; SearchTable, SearchSave if search_on; Open:Zone, TrigHoldoff. | keep | `ui/src/ui.rs` | Kept |
| menus-1001 | Trigger WHEN group | TrigWhen; TrigT1 if when != Off; TrigT2 if when is Inside/Outside. TrigT1 label is Lower limit in those two cases. | keep | `ui/src/ui.rs` | Kept |
| menus-1002 | Edge | TrigSrc, TrigSlope; on analogue source add TrigLevel, Trig50, TrigCoup. LA source has no per-line level. | keep | `ui/src/ui.rs` | Kept |
| menus-1003 | Protocol / Bus | DecProto, TrigBusWhat; TrigBusVal only for ID/Addr or Data; then serial group below. Decoder protocol/sources are shared with Decode, not copied. | keep | `ui/src/ui.rs` | Kept |
| menus-1004 | Pulse | TrigSrc, TrigPol, WHEN group, TrigLevel. | keep | `ui/src/ui.rs` | Kept |
| menus-1005 | Slope | TrigSrc, TrigSlope, WHEN group, TrigLevel (Upper), TrigLevel2 (Lower). | keep | `ui/src/ui.rs` | Kept |
| menus-1006 | Video | TrigSrc, VidStd, VidSync, VidLine if sync=Line n, TrigLevel. | keep | `ui/src/ui.rs` | Kept |
| menus-1007 | Pattern | Pat(0), Pat(1), Pat(2), Pat(3), TrigSrc (Level of), PatLevel, Open:LaPat if la_on(). | keep | `ui/src/ui.rs` | Kept |
| menus-1008 | Duration | Pat(0)..Pat(3), WHEN group, TrigSrc (Level of), PatLevel, Open:LaPat if la_on(). | keep | `ui/src/ui.rs` | Kept |
| menus-1009 | Timeout | TrigSrc, TrigSlope, TrigT1 (Timeout), TrigLevel. | keep | `ui/src/ui.rs` | Kept |
| menus-1010 | Runt | TrigSrc, TrigPol, WHEN group (Off permitted), TrigLevel (Upper), TrigLevel2 (Lower). | keep | `ui/src/ui.rs` | Kept |
| menus-1011 | Window | TrigSrc, WinModeI; WHEN group only for Inside; TrigLevel (Upper), TrigLevel2 (Lower). | keep | `ui/src/ui.rs` | Kept |
| menus-1012 | Delay | TrigSrc (Source A), TrigSlope (Slope A), TrigSrc2 (Source B), TrigSlope2 (Slope B), WHEN group, TrigLevel (Level A), TrigLevel2 (Level B). | keep | `ui/src/ui.rs` | Kept |
| menus-1013 | Setup/Hold | TrigSrc (Data), TrigSrc2 (Clock), TrigSlope2 (Clock edge), Setup, Hold, TrigLevel (Data level), TrigLevel2 (Clock level). | keep | `ui/src/ui.rs` | Kept |
| menus-1014 | Nth edge | TrigSrc, TrigSlope, NthIdle, NthN, TrigLevel. | keep | `ui/src/ui.rs` | Kept |
| menus-1015 | Protocol serial value group | SerRel, SerMax only if relation uses_max() (<, In range, Out of range); SerMask only when the protocol recipe requests a mask. I2C ID edits addr_rel/addr_max; other cases edit rel/max. | keep | `ui/src/ui/sertrig.rs` | Kept |
| menus-1016 | UART serial body | Value group without mask if Data; UartErr if Error; then UartPol, UartStop for all conditions. | keep | `ui/src/ui/sertrig.rs` | Kept |
| menus-1017 | SPI serial body | Value group with mask if Data; then SpiCs if dec_cs exists, otherwise SpiTimeout. | keep | `ui/src/ui/sertrig.rs` | Kept |
| menus-1018 | I2C serial body | ID: value group without mask, I2cAddrBits, I2cDir. Data: value group with mask, I2cBytes, I2cDir. Frame/Error: no serial rows. Other protocols: no serial rows. | keep | `ui/src/ui/sertrig.rs` | Kept |
| menus-1019 | Decode prefix | Decode: DecSel, DecOn, DecProto; DecIn if LA on and not C-PHY; DecSrc; role 1/2/3 source rows when declared; DecDiff if module diff_ok and not LA input. C-PHY uses required A/B/C and Min level gap. | keep | `ui/src/ui.rs` | Changed |
| menus-1020 | Decode protocol body | Protocol body: Protocol::shared in declaration order, then DecOpt(0..options.len()) in declaration order. Decoder registry above exhaustively gives source roles, shared rows and every production option domain; test Example is test-only (Bit rate and Show). | keep | `ui/src/ui.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-1021 | Decode suffix | Decode suffix: DecThr unless LA input; DecTable, DecExport; injected SrcOn only for off analogue primary DecSrc. Loaded references are stored records, not off analogue sources. | keep | `ui/src/ui.rs` | Changed |
| menus-1022 | Source-on injection | Source-on injection follows every menu body only if off_sources is nonempty. Spectrum/Eye use primary; Power Ripple/Rail V only, other pages V+I; Magnetic V+I; Inverter per page/angle plus optional mechanical sources; Dpt V/I/G plus enabled fourth; Bode Free output+optional input+sync, other generators input/output; BwTest/Mask use CH; Dvm uses enabled meter sources except offline; Decode primary only. Stored references never require channel enable. | keep | `ui/src/ui.rs` | Changed |
| menus-1023 | Picker contexts | Ordinary options: one PickOpt(k) per options(owner), same order. Hex: 0,1,2,3,4,5,6,7,8,9,A,B,C,D,E,F,Del,Clear,OK. Frequency: 7,8,9,mHz,4,5,6,Hz,1,2,3,kHz,0,.,Del,MHz,Clear,Cancel. WLAN/forget/key removal: dynamic list. No extra top-level menu variants for these contexts. | keep | `ui/src/ui.rs` | Kept |

Selecting a different trigger type or decoder protocol has legacy setup side effects (source enabling, demo defaults,
trigger selection, bus-entry restoration). These are retained, not silently redesigned: `ui/src/ui.rs` and
`ui/src/ui.rs`. The rebuild must emit the equivalent commands/events in the architecture's order. This spec
does not authorize unit calibration, generator output, updates or reboot during extraction.

## Shared value domains and indexed expansions

Domain value sets default to keep: these are literal legacy sets or capability/runtime-derived
sets. Their selectors inherit H1, including identity changes where stated. The baseline's comments are not authoritative where the implementation differs.

| ID | Domain | Ordered values / expansion | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|---|
| menus-1100 | D-VScale | 100, 200, 500 uV; 1,2,5,10,20,50,100,200,500 mV; 1,2,5,10 V/div for 1x probe. Multiply by probe and fine factor; fine steps follow board calibration rows, not an invented uniform ladder. | keep | `core/src/model/tables.rs`; `ui/src/ui.rs`; `ui/src/board/dho/mod.rs` | Kept |
| menus-1101 | T-scale | T-scale: 41 1-2-5 entries from 50 ps/div through 1000 s/div. Below 1 ns/div is a display scale for stored references/SIM; live ADC uses HW_MIN_TDIV=1 ns and floors live time/div when activated. Fine time follows fine_time(); record fit may also use a subminimum fine factor. | keep | `core/src/model/tables.rs`; `ui/src/ui/offline.rs` | Changed |
| menus-1102 | T-trigger | 1-2-5 from 1 ns through 10 s inclusive; nearest-at-or-above search then clamped signed step. | keep | `core/src/model/trigger.rs`; `ui/src/ui.rs` | Kept |
| menus-1103 | F-ladder | 0, then 1-2-5 per decade from 1 through 5e9; 0 means Full/Auto where stated. Implementation upper value is 5 GHz despite comment saying 1 GHz. | keep | `ui/src/ui/analysis.rs` | Kept |
| menus-1104 | D-MathScale | 1-2-5 per decade from 1e-9 through 5e9 units/div; FFT separately uses 1,2,5,10,20 dB/div. Scale is selected by clamped index. | keep | `core/src/math.rs`; `core/src/model/tables.rs`; `ui/src/ui.rs` | Kept |
| menus-1105 | D-Coupling | DC, AC, GND in current DHO adapter; another board supplies its own list. Trigger coupling is separately DC, AC, LF reject, HF reject. | keep | `ui/src/board/dho/mod.rs`; `core/src/model/channel.rs` | Kept |
| menus-1106 | D-BW | Auto, Full, 20 MHz, GND, 5,10,15,25,30,35,40,50,60,70,75,80,90,100,125,150,175,200,250,300,310,400,600,800 MHz, 1,1.5,2 GHz, in precisely that order. These are menu choices, not a verified bandwidth claim for every model. | keep | `core/src/model/tables.rs` | Kept |
| menus-1107 | D-Depth | Current DHO: Auto, 1k, 10k, 100k, 1M, 10M, 25M, 50M, 62.5M, 100M, 125M samples. Selected depth index comes from board.depths(); effective depth is channel/capability capped. | keep | `ui/src/board/dho/mod.rs`; `ui/src/model.rs`; `ui/src/ui.rs` | Kept |
| menus-1108 | D-Baud / D-CAN / D-SENT | Baud: 1200,2400,4800,9600,19200,38400,57600,115200,460800,921600 bit/s. CAN: 125k,250k,500k,800k,1M bit/s. SENT: 3,5,10,20,30,50,90 us, then Auto (0). | keep | `core/src/decode.rs` | Kept |
| menus-1109 | D-Math | D-Math: family order Arithmetic, Function, Filter, Logic, Convert. Each operation picker prepends Off. Arithmetic: A+B, A-B, AxB, A/B, Conv. Function: Intg, Diff, Abs, Sqrt, Lg, Ln, Exp, Ax+B, Formula. Filter: Low-pass, High-pass, Band-pass, Band-stop, FIR, FFT. Logic: AND, OR, XOR, NAND, NOR, NOT. Convert: A2D, D2A, NRZI inv. M1..M8 remain independently stored. | keep | `core/src/math.rs`; `ui/src/ui.rs` | Changed |
| menus-1110 | D-Protocols | D-Protocols: UART, I2C, SPI, CAN, LIN, Parallel, C-PHY, DMX512, S/PDIF, USB PD, DCC, Manchester, MLT-3, UART 9-bit, Modbus RTU, Modbus ASCII, 1-Wire, DS2482, SWD, JTAG, Quad-SPI, I3C, CAN-FD, FlexRay, SENT, ARINC 429, 1553, I2S, USB. Production count 29 with default features; optional Cargo features may remove entries. System > Features further filters the offered picker, not the availability pages. Example appended only in tests. | keep | `ui/src/protocols/mod.rs` | Changed |
| menus-1111 | D-Modules | D-Modules: production module options now exist. Decoder registry lists each DecOpt(k), choices, order and default. Runtime fits stored indices to domain length; choosing a changed non-reference protocol resets its module options; existing reference-source context retains fitted options. Test Example offers Bit rate=9600/19200/115200 (default 1) and Show=Hex/Decimal (default 0). | keep | `ui/src/ui.rs`; `ui/src/protocols/example.rs` | Changed |
| menus-1112 | D-Harmonics / D-Trend | Harmonics: All,5,10,20,50. Trend: Auto, Off, then current measurement rows; singles prepend CHn, pairs already include sources. Chart 1 then Chart 2. | keep | `ui/src/ui/panes.rs` | Kept |
| menus-1113 | D-Pairs | D-Pairs: original eight Delay/Phase rise/fall combinations retain indices 0..7; append Skew rise-rise/fall-fall/rise-fall/fall-rise (8..11), Setup time, Hold time, Setup+Hold, Setup fall, Hold fall (12..16). Membership is ordered A-to-B W-source; same-source rejected. | keep | `ui/src/ui/tools.rs` | Changed |
| menus-1114 | D-Measurements | D-Measurements: 60 standard definitions, 17 pair definitions and 83 module definitions in the registered production catalogue (160 unique Def ids). Each tile is listed above with exact name, binding, placements and source. Added selections are capped at 1024 across m.meas + m.meas2 + m.cat; a full selection is rejected explicitly, with no oldest-row eviction. Registry group order determines tiles; legacy ALL_MEAS order is only the Add meas fallback. | keep | `ui/src/measures/mod.rs`; `ui/src/ui/tools.rs`; `core/src/model/wave.rs` | Changed |
| menus-1115 | D-LAThreshold | TTL 1.4 V; CMOS 5 V: 2.5 V; CMOS 3.3 V: 1.65 V; LVCMOS 2.5 V: 1.25 V; LVCMOS 1.8 V: 0.9 V; LVCMOS 1.2 V: 0.6 V; ECL -1.3 V; User. Separate preset/user value for D0-D7 and D8-D15 pods. | keep | `core/src/la.rs`; `ui/src/ui/la.rs` | Kept |
| menus-1116 | D-Generators | Built-in, Rigol DG, Siglent SDG, SCPI, Free-running sweep, UNI-T UTG900. Generator capability and transport validation remain in bode_begin(); this list is not a claim of physical verification. | keep | `ui/src/ui/bode.rs` | Kept |
| menus-1117 | D-Quick | Screenshot, Save PNG, Save setup, Save CSV, Autoset, Single, Run/Stop, Clear, Next view. QuickRun displays the configured action inline today. | keep | `core/src/model/display.rs`; `ui/src/ui.rs` | Kept |
| menus-1118 | D-NTP / D-Zone / D-Name | NTP options are SERVERS[0..6) in source order, custom current server if absent, then Other. Zone options: DHCP then clock::ZONES labels, custom current POSIX string if absent, then Other. Name: shortened default, custom if distinct, Other; value may include effective mDNS suffix/conflict suffix. Custom values are runtime strings, not fixed choices. Literal host presets remain referenced in source to respect repository privacy rules. | change H1 identity; keep value sets | `ui/src/ui/time.rs`; `ui/src/ui/remote.rs` | Kept |
| menus-1119 | D-Clock | board.clock_steps() = target ACTIVE.clock_steps, offered in target order, rendered with overclock::label. Current DHO steps (code, not verified on unit): 1.25,1.375,1.5,1.75,2 GHz, last labelled (test). Selected next-boot frequency is separate from current runtime clock; the knob clamps rather than wraps. | keep | `ui/src/board/dho/mod.rs`; `ui/src/ui/overclock.rs`; `hal/src/dho/target.rs` | Changed |
| menus-1120 | Segments | seg_limit() is min(board.seg_max,100000) for supported hardware segments on hardware source, otherwise software limit 1000. Filter SEGMENTS to this limit and append the limit if absent. seg_count() is min(requested,limit). Interval chooser is its eight listed entries, not the broader 10 s normalization ceiling. | keep | `ui/src/ui.rs` | Kept |
| menus-1121 | Decoder source role labels | Decoder source role labels are the four slots in the Decoder registry above. A dash suppresses that row. Shared Row names map to their Dec* items; role 2 is optional except C-PHY C. LA uses pod threshold; reference decoder values are physical stored samples with their own intervals. | keep | `ui/src/ui.rs`; `ui/src/protocols/mod.rs` | Changed |
| menus-1122 | Differential source eligibility | Differential eligibility is each module diff_ok flag in the Decoder registry. The differential row is suppressed for LA input. None disables subtraction; an analogue/reference B source is otherwise picked from decoder_sources, using configured source validation. | keep | `ui/src/ui.rs`; `ui/src/ui/reference_decode.rs` | Changed |
| menus-1123 | LA trigger eligibility | D0..D15 join analogue source choices only for an LA-capable/on configuration and Edge or Pulse trigger type. On an LA Edge source no analogue level/coupling/50% row appears. | keep | `core/src/model/acquisition.rs`; `ui/src/ui/la.rs`; `ui/src/ui.rs` | Kept |
| menus-1124 | Hex storage | Value and mask input cap at 40 bits. SerMax for an I2C ID clamps further to 0x3ff and stores u32; normal SerMax stores u64. An all-ones 40-bit mask stores u64::MAX; display masks back to 40 bits. Keypad Back cancels temporary edits. | keep | `ui/src/ui/sertrig.rs`; `ui/src/ui.rs` | Kept |

### Indexed Item expansion audit

All payload variants of `Item` are resolved here; ranges are half-open and retain source order.
This table clarifies the existing requirement rows without allocating new ids. The enums and the
exhaustive `areas!` registration in `ui/src/ui/menu.rs` account for 457 Item variants and 48 MenuId
variants. Counts of rendered controls depend on capabilities, state and the selected registry entries.

| Item family | Exact expansion and order | Current source |
|---|---|---|
| `Open(menu)` | The 48 MenuId variants above; `Channel(c)` and `ChMore(c)` for c in 0..n_ch(); `Decoders(k)` for successive 18-entry registry pages, last link returns to page 0. Pick is an internal context. | `ui/src/ui/menu.rs`, `ui/src/ui/features.rs` |
| `ChOn(c)`, `ChCoupling(c)`, `ChBw(c)`, `ChProbe(c)`, `ChInvert(c)`, `ChScale(c)`, `ChOffset(c)`, `ChLabel(c)`, `ChUnit(c)`, `ChBias(c)`, `ChBiasZero(c)`, `ChSkew(c)` | c in 0..n_ch(), with Channel/ChMore row order as listed. Includes off channels; no CH3/CH4 menu on two-channel models. | `ui/src/ui/menu/channel.rs` |
| `Pat(c)` | c in 0..4, in order, for Pattern/Duration even on two-channel models; distinct from LA pattern lines. | `ui/src/ui/menu/trigger/rows.rs` |
| `RefShow(i)` | i in 0..NREF (10), including empty slots, after the Reference settings rows. | `ui/src/ui/menu/reference.rs` |
| `MeasAdd(kind)` | Standard measurement catalogue bindings listed above; registry/category/group order controls tiles. MeasAdd's generic items() remains empty. ALL_MEAS order is only the Add meas fallback. | `ui/src/ui/meascatalog.rs`, `ui/src/ui/menu/measure.rs` |
| `MeasPair(k)` | k in 0..PAIRS.len() (17), preserving the eight original indices and nine appended pairs. | `ui/src/ui/menu/measure.rs`, `ui/src/ui/tools.rs` |
| `MeasRemove(index)`, `MeasCfg(index)` | Combined selection indices enumerate singles, pairs, then module rows; current page covers 16 indices, clamped to the last page. Remove exists for each row; Set uses the same index only for singles. The custom layout interleaves Remove/Set, while generic items() contains Remove only. Catalogue Set targets the corresponding selected single. Total selection cap is 1024. | `ui/src/ui/measurement_nav.rs`, `ui/src/ui/meascatalog.rs`, `ui/src/ui/tools.rs` |
| `WinCh(c)`, `WinRef(i)`, `WinMath(k)` | Only with more than one window: enabled channels in 0..n_ch(), then populated On references in 0..10, then non-Off math channels in 0..8, then WinLa if LA is on. | `ui/src/ui/windows.rs` |
| `TrendSel(k)` | k=0 then 1, inserted after Open:Windows only when pane=Trend; source labels follow the current measurement rows after Auto, Off. | `ui/src/ui/menu/display.rs`, `ui/src/ui/panes.rs` |
| `DecOpt(k)` | k in 0..selected protocol options.len(), after shared protocol rows. Each protocol's exact domains/defaults are in Decoder registry; optional Cargo features remove whole protocols. | `ui/src/ui/menu/decode.rs`, `ui/src/protocols/` |
| `FeatOn(i)` | Analyser registry order (Spectrum, Eye, Power, Bode, Bandwidth, Magnetic, Inverter, Double pulse), then SIM, Load file: i in 0..10. Availability rows include disabled features. | `ui/src/ui/features.rs`, `ui/src/ui/analysers.rs` |
| `FeatDec(i)` | Absolute compiled PROTOS indices, page k offers k*18..min((k+1)*18,PROTOS.len()); 29 production protocols with default features, test Example appended in tests only. Availability pages include disabled protocols. | `ui/src/ui/features.rs`, `ui/src/protocols/mod.rs` |
| `IdFind(k)` | k in 0..min(findings.len(),8), following IdentifyAct and ProtoWatch. Finding descriptions are momentary actions, not committed choices. | `ui/src/ui/analysis.rs` |
| `LaThr(p)`, `LaUser(p)` | p=0 then 1; each pod threshold immediately followed by its User row only when that pod selects User. | `ui/src/ui/la.rs` |
| `LaLine(n)`, `LaPat(n)` | n in 0..16, ascending D0..D15. Line selection and trigger patterns use separate menus and value sets. | `ui/src/ui/menu/logic.rs`, `ui/src/ui/la.rs` |
| `PickOpt(k)` | k in 0..options(owner).len(), in owner option order: ordinary choices, dynamic lists, 19 hex keys or 18 frequency keys as listed in Pick. Picker contexts add no MenuId variants. | `ui/src/ui/menu/picker.rs` |

## Data model contract

| ID | Requirement | Disposition | Evidence | Delta vs e9f7c30b |
|---|---|---|---|---|
| menus-60 | A menu item is data: stable id, label/role reader, one of the documented control kinds, typed value reader, command/event producer, ordered option identities and labels, visibility/enable predicate, active identity and optional submenu summary/target. One renderer uses these fields. Numeric step/range and status-only rows are explicit; no per-menu renderer decides whether selected state exists. | change representation under accepted ADR 0038; preserve inventory behaviours | `ui/src/ui.rs`; [ADR 0038](../adr/0038-menus-and-parser.md) | Kept |
| menus-61 | Effective displayed view uses XY precedence, then m.view; retained analyser settings remain inspectable when another view is displayed. The view entry's active cue names the displayed view. Expose a retained/suppressed analyser preference separately if needed instead of falsely labelling it the displayed view. The owner question below resolves the legacy simultaneous XY/analyser case. | change H1: effective view indication | `ui/src/ui/viewmore.rs`; `ui/src/ui/analysis.rs`; `ui/src/ui/bode.rs`; `ui/src/ui/bwtest.rs` | Kept |

## Review and owner questions

The extraction and proposed H1/N1 dispositions need coordinator review; the menu tree, highlight rule and overview
need owner approval under ADR 0038. Read-side migration starts with DVM/counter; other areas still use E1 read dispatches.
The owner proposals below are not part of this representation-only package. These are explicit open owner questions, not
changes already accepted:

- **menus-1900:** Approve N1's focusable Back/Close and full parent-chain return, including the added focus stops.
  The owner requirement says to define navigation; these are the proposed concrete return paths.
- **menus-1901:** Approve filled active-value cues for collapsed pickers and inline parent summaries. Should large
  summaries wrap to two lines or use a short label plus details on entry? Preserve full values in the destination.
- **menus-1902:** Confirm keeping legacy opening side effects: Cursor Off becomes Time, channel/LA key can enable
  or disable its input, and selecting a trigger/protocol can apply demo setup defaults. They are preserved here,
  but differ from ordinary read-only navigation.
- **menus-1903:** Confirm keeping legacy unfiltered CH1..CH4 Pattern/Duration rows
  on two-channel models. Self-cal now offers All plus existing channels only; preserve that upstream change.
- **menus-1904:** Confirm the submenu summary wording and the overview entry/layout proposals in overview.md.
  Ten removed/superseded surface rows are tombstones with drop disposition; other drops need a recorded rationale and approval.

- **menus-1905:** When XY is enabled while m.view retains an analyser, approve showing XY as the active displayed
  view with the analyser marked retained/suppressed. Should selecting an analyser's On option also leave XY?
  The current code does not; this draft changes the active indication and leaves that selection side effect open.

- **menus-1906:** Approve the merged 48-menu surface and the additional catalogue/context/modal rows, including
  the current relocation of SIM and Load file. The upstream summary mentioned top buttons, but the baseline places
  SIM in Main and Load file in Reference. This draft keeps those locations.
- **menus-1907:** Engineering ladder E records exact emitted sets and the absolute 1 ps anchor tolerance, including
  bounds beyond helper arguments. These do not conflict with the owner's presentation/navigation rules, so their
  default disposition is keep. Any numerical correction needs a separately approved behaviour change.
- **menus-1908:** Approve the N1 extension to catalogue tiles, category/group navigation, Selected paging, popups
  and measurement configuration, whose generic Multi paths are incomplete today. Load destination selection is no
  longer part of it: its modal is the tombstone menus-1618.
