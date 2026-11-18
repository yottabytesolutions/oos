# One change point classifies settings and requests

- Number: 0033
- Category: code
- State: target: M4, M13, M14
- Date: 2026-11-18

## Context

The original audit found 39 settings_changed/mark_changed calls, 16 dirty_map assignments and hw_stamp bumps across UI,
Bode and autoset. A writer could update one mechanism and miss another, retaining old frames, statistics or segments
after a settings change. One classification point must include both value changes and requests.

## Decision

Once per step, apply commands, normalize, then compute ChangeSet = classify(previous, current) | requests. Only this
point decides reactions. classify destructures every group without ..; a field needs an explicit class. Internal writers
queue next-step commands and requests. CLEAR, RUN, SINGLE and epoch events return as requests even without a settings
difference. Generation and epoch counters are private to the change stage.

Check: compiler destructuring; host field/class tests check exactly which consumers reset.

## Alternatives

Keep independent dirty flags and stamps: rejected because each writer must remember every affected consumer. Infer all
requests from settings differences: rejected because CLEAR, RUN, SINGLE and epoch events can occur without a value
change.

## Consequences

M2 classification in core/src/change is in force. Removal of mark_changed, dirty_map and hw_stamp is M4; command
ownership follows M13/M14. Token protocol lives in architecture/README.md.
