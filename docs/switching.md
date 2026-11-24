# Switch between upstream OOS and this fork

Draft outline for the unit round. The two forks share the card layout and the package format. Their signing
trust is separate.

Upstream sees the shared `/oos` files on an ext3-compatible filesystem when this fork has enabled the journal.

## Upstream to this fork

1. Verify the labelled bridge against the authenticated release `SHA256SUMS`.
   That file is the trust anchor for the bridge. **To verify on the unit.**
2. Check the recorded build stamp in upstream's `oos-update status` against the bridge's
   build stamp. **To verify on the unit.**
3. Upload the bridge through upstream's web update page. **To verify on the unit.**
4. Check the kexec start, cold trial and confirmation. Check settings, calibration and the rollback
   slot. **To verify on the unit.**
5. Run `oos-update status`. When the state on the card recorded another recovery image and no trial or staged
   candidate was open, it reports `untracked` with the mismatch it saw. The next signed install then needs no
   `--downgrade` for that old stamp. A build this fork tracked itself keeps the normal downgrade guard.
   Later installs accept only this fork's release key. **To verify on the unit.**

## This fork to upstream

1. Finish any open update, then run `oos-update leave`. It clears the installed build stamp and the rollback
   tracking, nothing else. The next start still follows the boot selection. **To verify on the unit.**
2. [Start stock](install.md#return-to-stock). **To verify on the unit.**
3. Put upstream's GEL and an empty `oos-reinstall.txt` on the USB stick. Run **Storage > Upgrade**
   from stock. **To verify on the unit.**
4. Boot upstream, check settings and calibration, and dry-run its first signed update with
   `oos-update install FILE.oosupd`. **To verify on the unit.**

## Return to stock

1. Select **System > Boot mode > One-shot** in the running OOS build, or run `oos-update mode oneshot`.
   **To verify on the unit.**
2. Power-cycle and confirm stock starts. **To verify on the unit.**
