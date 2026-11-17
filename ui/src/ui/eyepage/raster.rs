// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// The coloured hits into `r`. Same size is copied; a column that is mid-slide is scaled so the eye stays put. Both go
/// through `Canvas::image` (0 is transparent), so a recording canvas (GPU drawing) gets them too.
pub(super) fn blit_hits(cv: &mut Canvas, r: Rect, img: &[u32], iw: usize) {
    if iw == 0 || r.w <= 0 || r.h <= 0 || img.len() < iw {
        return;
    }
    let ih = img.len() / iw;
    let (w, h) = (r.w as usize, r.h as usize);
    if iw == w && ih == h {
        return cv.image(r, &img[..iw * ih], true);
    }
    let mut px = vec![0u32; w * h];
    for (y, row) in px.chunks_exact_mut(w).enumerate() {
        let sy = (y * ih / h).min(ih - 1);
        let src = &img[sy * iw..(sy + 1) * iw];
        for (x, d) in row.iter_mut().enumerate() {
            *d = src[(x * iw / w).min(iw - 1)];
        }
    }
    cv.image(r, &px, true);
}
