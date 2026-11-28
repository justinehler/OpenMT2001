//! Frontend list geometry in the original 640×480 coordinate system.

// 430b50 reads selection at 536294 and hover RGB at 5362ac. Keep the
// existing fixed hover opacity; the original pulses its alpha via 4b4e90.
pub const SELECTED_COLOR: u32 = 0xff288be1;
pub const HOVER_COLOR: u32 = 0x996ed1e1;

pub fn row_color(selected: bool, hovered: bool) -> Option<u32> {
    if selected {
        Some(SELECTED_COLOR)
    } else {
        hovered.then_some(HOVER_COLOR)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    bounds: [f32; 4],
    pub pitch: f32,
    inset: f32,
    horizontal_inset: f32,
}

impl Layout {
    /// 4406a0 measures W and adds the text payload's signed line gap.
    /// Frontend RichTextRect constructors set gap=2 and vertical inset=3.
    pub fn from_font(bounds: [f32; 4], tab: &[u8]) -> Result<Self, String> {
        Self::from_font_with_insets(bounds, tab, [0., 3.])
    }

    /// Load Game configures horizontal inset0 while Tutorial/Scenario use3.
    pub fn from_font_with_insets(
        bounds: [f32; 4],
        tab: &[u8],
        insets: [f32; 2],
    ) -> Result<Self, String> {
        if !insets[0].is_finite() || insets[0] < 0. {
            return Err("invalid frontend list horizontal inset".into());
        }
        let at = 4 + usize::from(b'W' - 32) * 44 + 36;
        let height = i32::from_le_bytes(
            tab.get(at..at + 4)
                .ok_or("list font W height metric")?
                .try_into()
                .unwrap(),
        );
        let mut layout = Self::new(bounds, height as f32 + 2., insets[1])?;
        layout.horizontal_inset = insets[0];
        Ok(layout)
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
            horizontal_inset: 0.,
        })
    }

    /// 440540/440950 count whole pitches in the viewport, before the inset.
    pub fn visible_rows(self) -> usize {
        ((self.bounds[3] - self.bounds[1]) / self.pitch).floor() as usize
    }

    pub fn text_bounds(self, row: usize) -> [f32; 4] {
        let top = (self.bounds[1] + self.inset).trunc() + row as f32 * self.pitch;
        // 4419f0 submits complete glyphs for each visible row. A row's
        // advance is not its clip height: g/p/y extend below the W metric.
        [self.bounds[0], top, self.bounds[2], self.bounds[3]]
    }

    /// 4406a0 style0 adds two units after the horizontal text inset.
    pub fn text_origin(self, row: usize) -> [f32; 2] {
        [
            self.bounds[0].trunc() + self.horizontal_inset + 2.,
            self.text_bounds(row)[1],
        ]
    }

    /// 430b50 paints both bars at inset x, one below y, one short of pitch.
    pub fn highlight(self, row: usize) -> [f32; 4] {
        let top = self.bounds[1] + self.inset + row as f32 * self.pitch + 1.;
        [
            self.bounds[0] + self.horizontal_inset,
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
    fn load_preserves_zero_horizontal_inset_and_shared_vertical_layout() {
        let mut tab = vec![0; 4 + 96 * 44];
        let at = 4 + usize::from(b'W' - 32) * 44 + 36;
        tab[at..at + 4].copy_from_slice(&9i32.to_le_bytes());
        let layout = Layout::from_font_with_insets([20., 40., 284., 302.], &tab, [0., 3.]).unwrap();
        assert_eq!(layout.text_origin(0), [22., 43.]);
        assert_eq!(layout.highlight(0), [20., 44., 283., 54.]);
        assert_eq!(layout.text_bounds(0)[3], 302.);
        assert_eq!(layout.visible_rows(), 23);
        assert_eq!(row_color(true, true), Some(0xff288be1));
        assert_eq!(row_color(false, true), Some(0x996ed1e1));
        assert_eq!(row_color(false, false), None);
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
