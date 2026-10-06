//! Geometry shared by native input adapters. Coordinates use one logical space.

/// Center of the positive visible intersection; never clamp an offscreen target
/// onto unrelated content. Reject non-finite and degenerate rectangles.
pub fn visible_target_center(target: [f64; 4], viewport: [f64; 4]) -> Option<(f64, f64)> {
    for rect in [target, viewport] {
        if !rect.iter().all(|n| n.is_finite()) || rect[2] <= 0.0 || rect[3] <= 0.0 {
            return None;
        }
    }
    let left = target[0].max(viewport[0]);
    let top = target[1].max(viewport[1]);
    let right = (target[0] + target[2]).min(viewport[0] + viewport[2]);
    let bottom = (target[1] + target[3]).min(viewport[1] + viewport[3]);
    if !right.is_finite() || !bottom.is_finite() || right <= left || bottom <= top {
        return None;
    }
    Some((left / 2.0 + right / 2.0, top / 2.0 + bottom / 2.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn centers_visible_part_of_long_document() {
        assert_eq!(
            visible_target_center([20., 100., 600., 1200.], [0., 0., 690., 628.]),
            Some((320., 364.))
        );
        assert_eq!(
            visible_target_center([20., 20., 40., 60.], [0., 0., 100., 100.]),
            Some((40., 50.))
        );
    }
    #[test]
    fn refuses_unavailable_or_invalid_target() {
        for rect in [
            [101., 20., 40., 60.],
            [100., 0., 2., 2.],
            [0., 0., 0., 2.],
            [f64::NAN, 0., 2., 2.],
        ] {
            assert_eq!(visible_target_center(rect, [0., 0., 100., 100.]), None);
        }
    }
}
