//! Frontend list geometry in the original 640×480 coordinate system.

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    bounds: [f32; 4],
    pub pitch: f32,
    inset: f32,
}

impl Layout {
    /// 4406a0 measures W and adds the text payload's signed line gap.
    /// Frontend RichTextRect constructors set gap=2 and vertical inset=3.
    pub fn from_font(bounds: [f32; 4], tab: &[u8]) -> Result<Self, String> {
        let at = 4 + usize::from(b'W' - 32) * 44 + 36;
        let height = i32::from_le_bytes(
            tab.get(at..at + 4)
                .ok_or("list font W height metric")?
                .try_into()
                .unwrap(),
        );
        Self::new(bounds, height as f32 + 2., 3.)
    }

    pub fn new(bounds: [f32; 4], pitch: f32, inset: f32) -> Result<Self, String> {
        if bounds.into_iter().any(|v| !v.is_finite())
            || bounds[2] <= bounds[0]
            || bounds[3] <= bounds[1]
            || !pitch.is_finite()
            || pitch <= 0.
            || !inset.is_finite()
            || inset < 0.
        {
            return Err("invalid frontend list layout".into());
        }
        Ok(Self {
            bounds,
            pitch,
            inset,
        })
    }

    /// 440540/440950 count whole pitches in the viewport, before the inset.
    pub fn visible_rows(self) -> usize {
        ((self.bounds[3] - self.bounds[1]) / self.pitch).floor() as usize
    }

    pub fn text_bounds(self, row: usize) -> [f32; 4] {
        let top = (self.bounds[1] + self.inset).trunc() + row as f32 * self.pitch;
        [self.bounds[0], top, self.bounds[2], top + self.pitch]
    }

    /// 42f740 paints the selection one unit below the row, one short of pitch.
    pub fn highlight(self, row: usize) -> [f32; 4] {
        let top = self.bounds[1] + self.inset + row as f32 * self.pitch + 1.;
        [
            self.bounds[0],
            top,
            self.bounds[2] - 1.,
            top + self.pitch - 1.,
        ]
    }

    pub fn hit(self, point: [f32; 2], first: usize, count: usize) -> Option<usize> {
        let [l, t, r, b] = self.bounds;
        if !point.into_iter().all(f32::is_finite)
            || point[0] < l
            || point[0] >= r
            || point[1] < t
            || point[1] >= b
        {
            return None;
        }
        // 42f740 subtracts the three-unit inset before native truncation.
        let row = ((point[1] - t - self.inset) / self.pitch).max(0.) as usize;
        let index = first.checked_add(row)?;
        (row < self.visible_rows() && index < count).then_some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_metric_spacing_and_inset_drive_all_row_geometry() {
        let mut tab = vec![0; 4 + 96 * 44];
        let at = 4 + usize::from(b'W' - 32) * 44 + 36;
        tab[at..at + 4].copy_from_slice(&10i32.to_le_bytes());
        let list = Layout::from_font([20., 95., 300., 195.], &tab).unwrap();
        assert_eq!(list.pitch, 12.);
        assert_eq!(list.visible_rows(), 8);
        assert_eq!(
            (0..7).map(|i| list.text_bounds(i)[1]).collect::<Vec<_>>(),
            [98., 110., 122., 134., 146., 158., 170.]
        );
        assert_eq!(list.highlight(1), [20., 111., 299., 122.]);
        for row in 0..8 {
            let y = 98. + row as f32 * 12.;
            assert_eq!(list.hit([30., y + 5.], 4, 30), Some(4 + row));
            assert_eq!(list.hit([30., y + 11.9], 4, 30), Some(4 + row));
        }
        assert_eq!(list.hit([30., 194.], 0, 30), None);
        assert_eq!(list.hit([30., 114.], 0, 1), None);
        for point in [
            [19., 100.],
            [300., 100.],
            [30., 94.],
            [30., 195.],
            [f32::NAN, 100.],
        ] {
            assert_eq!(list.hit(point, 0, 30), None);
        }
        // A different font must change capacity and input as well as drawing.
        tab[at..at + 4].copy_from_slice(&9i32.to_le_bytes());
        let list = Layout::from_font([20., 95., 300., 195.], &tab).unwrap();
        assert_eq!((list.pitch, list.visible_rows()), (11., 9));
        assert_eq!(list.hit([30., 110.], 0, 30), Some(1));
    }

    #[test]
    #[ignore = "requires original frontend font tables"]
    fn original_frontend_fonts_keep_their_distinct_pitches() {
        let installer = crate::fixtures::path("INSTALLER");
        for (font, pitch) in [
            ("tahoma11noantip.tab", 12.),
            ("tahoma10bldnoantip.tab", 11.),
        ] {
            let tab = std::fs::read(installer.join("Standard/fonts").join(font)).unwrap();
            let layout = Layout::from_font([20., 95., 300., 195.], &tab).unwrap();
            assert_eq!(layout.pitch, pitch, "{font}");
        }
    }

    #[test]
    fn invalid_or_truncated_layouts_are_rejected() {
        assert!(Layout::from_font([0., 0., 100., 100.], &[]).is_err());
        assert!(Layout::new([0., 0., 100., 100.], 0., 3.).is_err());
        assert!(Layout::new([0., 100., 100., 0.], 12., 3.).is_err());
    }
}
