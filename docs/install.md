# Install OOS

Draft outline for the unit round.

## First install from stock

1. Obtain the project GEL and verify it against the release `SHA256SUMS`. **To verify on the unit.**
2. Put the GEL and `oos-dry-run.txt` on the USB stick. Select **Storage > Upgrade** in stock,
   confirm the stock system extracts the GEL with epoch-zero member modification times, then check the install
   log and rescue set. **To verify on the unit.**
3. Remove `oos-dry-run.txt` and repeat **Storage > Upgrade**. **To verify on the unit.**
4. Check the first OOS boot, build, calibration and settings. **To verify on the unit.**

## Update a running OOS installation

1. Verify the project-signed update package against the release `SHA256SUMS`. **To verify on the unit.**
2. Dry-run `oos-update install FILE.oosupd`. **To verify on the unit.**
3. Upload the package through the OOS web update page. **To verify on the unit.**
4. Check the trial, confirmation and boot mode in `oos-update status`. **To verify on the unit.**

## Return to stock

1. Select **System > Boot mode > One-shot**. **To verify on the unit.**
2. Power-cycle and confirm stock starts. **To verify on the unit.**
