# UI design rules

These rules collect the former ADR 0005 to 0008. They describe UI design, rather than architecture decisions.

## Measurements

There is one measurement page. Categories stand down the left (Basic, Two sources, Serial & PAM, Jitter & eye,
Power, Motor drive, Power rails, Double pulse, then Selected). Sources and category groups are to the right, one
open group at a time. The bottom describes the last tile tapped.

One tap adds a measurement; a second removes it. An enabled tile is filled. The Measure key, Measure button and
Analysis > Measure open the same page. Settings are grouped as Sources, Measure over, Display, Parameters and More.
Selected lists enabled measurements with Remove, Set and Clear all. The page covers the plot while open.

Each measurement has one full name and place, declared by its module (`Def::places`, ADR 0002). See
[measurement modules](measurement-modules.md) and the [analysis reference](reference-analysis.md).

## Lists

Every list longer than its window has a scroll bar and follows a finger drawn over it. Dragging the thumb scrolls;
pressing the track jumps there. Movement of less than 10 px along the list remains a tap on the item. There are no
Up / Down buttons. Clamp the position whenever it is read, including after a group folds or a list shrinks.

Use the shared `Bar`, `Grip` and `Drag::Scroll` implementation in `ui/src/ui/scroll.rs`. The bar has 40 px of touch
width (10 px drawn) and a thumb at least 40 px high.

## Top bar

The top bar holds Measure, Cursor, Display and Storage. Other functions start from the Menu button at bottom left:
System leads its group, SIM is under Analysis, and Load file belongs in Reference. A function gets a top-bar entry
only when it is used continually while measuring; a function belonging to one menu stays in that menu.

In record mode (offline waveforms, a taken-over record or SIM), Live appears before the four entries and returns to
acquisition. It disappears during live acquisition.

## Visual style

Keep the screen arrangement: plot, channel cards, time base and trigger at the edges. Use near-black backgrounds,
greys and white accents for selection, dialog frames and enabled tiles. Buttons, dialogs and cards have rounded
corners. Leave no unused black areas above or beside cards; text must fit with every channel, the logic analyser and
the generator enabled. Do not copy another product's look or arrangement.

Use the shared colour constants in `ui/src/ui.rs`. A page-specific colour is a named constant with a reason, rather
than a literal in drawing code. Check new pages at all three text sizes with everything enabled, following
[ADR 0014](adr/process/0014-refactorings-keep-every-pixel.md).
