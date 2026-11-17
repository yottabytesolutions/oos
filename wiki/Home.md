OOS replaces the software of a Rigol DHO800/DHO900 oscilloscope with a Linux 6.12 kernel and a new scope application
written in Rust. It runs from the scope's own SD card, next to the stock system, and you can switch back at any time.

![Two channels: a 10:1 probe on the calibration signal and the built-in generator](https://raw.githubusercontent.com/yottabytesolutions/oos/main/docs/images/channels.png)

## Safety first

OOS is experimental software. It has run on a DHO924S and, by a user report, on a DHO804; several functions have never
run on real hardware ([status](Supported-models-and-features#status)). Measurements made with OOS are not calibrated to any
standard.

- Installing with a second card means opening the scope, and that voids the warranty.
- Back up the original card before anything else. Without that copy, your factory calibration cannot be replaced.
- Connect the scope to a trusted bench network only, never directly to the internet.

Read the [[Disclaimer]] before you install. Installing OOS means you accept its risks.

## Pages

| Page | What it covers |
|---|---|
| [[Disclaimer]] | The risks you accept, in full |
| [[Supported models and features]] | Which models, what OOS does, what has run on real hardware |
| [[Gallery]] | Screenshots of a DHO924S running OOS |
| [[Try it without a scope]] | The desktop build and the demo signals |
| [[Installation]] | The three ways to install, and which to choose |
| [[First start and choosing how OOS starts]] | What the first start looks like, one-shot and persist |
| [[Daily use]] | Controls, views, analysers, web remote, files, updates |
| [[Troubleshooting]] | What you see, what it means, what to do |
| [[Going back to stock]] | Back to the stock system, for one start or for good |
| [[Building OOS]] | The kernel, the signing key and the image |
| [[Development]] | Where contributors start |
| [[FAQ]] | Short answers to common questions |
| [[Thanks and licences]] | The projects OOS builds on, and the licences |

## Where this wiki comes from

This wiki is generated from [`wiki/`](https://github.com/yottabytesolutions/oos/tree/main/wiki) in the repository.
Edits made here in the web interface are overwritten on the next publish. Propose changes with a pull request.
