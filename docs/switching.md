# Switch between upstream OOS and this fork

Draft outline for the unit round.

## Upstream to this fork

1. Verify the labelled bridge against the authenticated release `SHA256SUMS`.
   That file is the trust anchor for the bridge. **To verify on the unit.**
2. Check the recorded build stamp in upstream's `oos-update status` against the bridge's
   build stamp. **To verify on the unit.**
3. Upload the bridge through upstream's web update page. **To verify on the unit.**
4. Check the kexec start, cold trial and confirmation. Check settings, calibration and the rollback
   slot. **To verify on the unit.**

## This fork to upstream

1. [Start stock](install.md#return-to-stock). **To verify on the unit.**
2. Put upstream's GEL and an empty `oos-reinstall.txt` on the USB stick. Run **Storage > Upgrade**
   from stock. **To verify on the unit.**
3. Boot upstream, check settings and calibration, and dry-run its first signed update with
   `oos-update install FILE.oosupd`. **To verify on the unit.**

## Return to stock

1. Select **System > Boot mode > One-shot** in the running OOS build. **To verify on the unit.**
2. Power-cycle and confirm stock starts. **To verify on the unit.**
