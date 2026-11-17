Read this before you install OOS. Installing it means that you accept these risks.

**Use OOS entirely at your own risk.** OOS is provided "as is", without warranty of any kind, express or implied,
including but not limited to merchantability, fitness for a particular purpose and non-infringement. To the maximum
extent permitted by applicable law, the authors and contributors are not liable for any damage or loss arising from
its use or from following these instructions. That includes damage to the instrument, to connected equipment or
devices under test, loss of data or calibration, lost warranty, wrong measurements, injury and any consequential loss.

## The risks

Installing OOS means that you accept in particular the following risks:

- **Warranty.** The boot card is inside the instrument. **Installing OOS with a second card means opening the scope,
  and that voids the warranty.** The USB install does not open it, but it changes the card, and nothing here promises
  that your warranty survives that. If it goes wrong, you may have to open the scope anyway.
- **Instrument damage.** OOS drives the hardware directly: FPGA, ADC, front ends, sample clock, fan, relays and the
  generator output. A software fault can leave hardware in a state the stock firmware would never select. A failed fan
  control or a wrong setting can cause overheating or damage.
- **Data loss.** A mistake while writing the card can make the scope unbootable or destroy the factory calibration
  stored on it. Without a good copy of your original card this data **cannot be replaced**.
- **Licences.** Option licences of the stock system have been seen to stop working after running OOS, and had to be
  entered again. Keep your licence keys at hand.
- **Wrong measurements.** OOS is not a calibrated or certified measuring instrument. Readings, trigger behaviour,
  decoders and analysis results can be wrong without any visible sign. Do not use OOS for safety-relevant, medical,
  legal, commercial or compliance measurements, and never rely on it to decide whether something is safe to touch.
- **Electrical safety.** OOS changes nothing about the input ratings of the instrument. Observe the manufacturer's
  safety instructions and ratings. The open instrument contains a mains power supply with dangerous voltages.
  Disconnect the mains and all probes before you open it, and close it again before you connect mains. Only open it if
  you know how to do this safely.
- **Network security.** Apart from SSH (password or key,
  [docs/network.md](https://github.com/yottabytesolutions/oos/blob/main/docs/network.md#7-ssh) section 7) OOS has no
  authentication: telnet (on by default, a shell without password), SCPI, VXI-11, HiSLIP (on a computer), the web
  remote and MCP are open to everyone who can reach the scope. Updates require a signature from this project's
  release key. The open services are reachable over WLAN as well. **Connect the scope to a trusted bench network
  only, never directly to the internet.**
- **Experimental settings.** System > Overclock and `/oos/limits.conf` can change the sample clock, `limits.conf` can
  also lift limits. Values outside the defaults are outside any rating and can make the scope unusable or damage it.
  Overclocked samples are worse and at 2 GHz wrong: see
  [docs/models.md](https://github.com/yottabytesolutions/oos/blob/main/docs/models.md).

## Not affiliated with Rigol

OOS is an independent project. It is not affiliated with, endorsed by or supported by RIGOL TECHNOLOGIES CO., LTD.
Rigol and all other names are trademarks of their respective owners and are used here only to identify the hardware.
Do not contact Rigol for support with OOS. The repository contains no Rigol code. At run time OOS uses the FPGA image
already in the scope's flash, the calibration files on its own card and the stock software on that card, which it
needs to read the scope's identity. It only reads them.
