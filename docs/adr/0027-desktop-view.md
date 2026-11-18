# The desktop view is the browser UI

- Number: 0027
- Category: code
- State: in force
- Date: 2026-11-18

## Context

The native Windows window was the only user of minifb and windows-sys, and no gate or CI job built it. The same screen
already ran behind oos-ui --serve with a demo source and browser remote on every supported host. Keeping two desktop
renderers would duplicate screen behaviour and leave one path untested.

## Decision

The browser remote UI is the desktop view on every supported host. On Windows, oos-ui runs in WSL and the Windows
browser shows the same UI. The native Windows window lives in the separate oos-desktop project; it is not built here.

Check: review only; host --serve smoke tests exercise the browser interface.

## Alternatives

Keep an unsupported native window: rejected because unbuilt code rots and costs review on every nearby change. Port the
window to Linux/macOS: rejected because it duplicates the browser view for the same users.

## Consequences

No native desktop frontend is maintained in this tree.
