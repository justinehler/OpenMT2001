//! Manager graph dialog state and plot geometry (native 49dcb0 show, 49cd40
//! metric, 49e630 actions, 42ccc0/42d220 plot, 42d970 lines). The binary's
//! `stats_panel` module draws it with graph_panel.tm.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub mode: usize,
    pub selected: [bool; 6],
    pub visible: [bool; 6],
}
impl State {
    ///49dcb0 shows every manager slot, regardless of its active accounting byte.
    /// Only stopped objective1/2 presentations expose the sixth market-share metric.
    pub fn show(objective: i32, running: bool, players: usize) -> Result<Self, String> {
        if players > 6 {
            return Err("Stats roster exceeds six native selectors".into());
        }
        let mode = match objective {
            1 | 2 if !running => 5,
            3 => 4,
            4 | 5 => 1,
            6 => 0,
            _ => 2,
        };
        Ok(Self {
            mode,
            selected: std::array::from_fn(|i| i < players),
            visible: std::array::from_fn(|i| i < players),
        })
    }
    ///49cd40 wraps at either boundary without a remainder operation.
    pub fn set_mode(&mut self, value: i32) {
        self.mode = if value < 0 {
            5
        } else if value > 5 {
            0
        } else {
            value as usize
        };
    }
    ///49e630 checks live running/objective values when an arrow is activated.
    pub fn cycle(&mut self, right: bool, objective: i32, running: bool) {
        let all = !running && matches!(objective, 1 | 2);
        let value = self.mode as i32 + if right { 1 } else { -1 };
        if all {
            self.set_mode(value);
        } else {
            self.set_mode(if right && value == 5 {
                0
            } else if !right && self.mode == 0 {
                4
            } else {
                value
            });
        }
    }
    pub fn toggle(&mut self, player: usize) -> Result<(), String> {
        if !self.visible.get(player).copied().unwrap_or(false) {
            return Err("Stats selector leaves visible manager slots".into());
        }
        self.selected[player] = !self.selected[player];
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub character: u32,
    pub name: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub state: State,
    pub players: Vec<Player>,
    /// Unmodified retained20000 history pool. Assets supply original graph
    /// bounds; the worker retains host-free data rather than a GPU object.
    pub history: Vec<u8>,
    pub years_per_day: i32,
}
impl Snapshot {
    pub fn from_session(
        session: &crate::session::Session<'_>,
        state: &State,
    ) -> Result<Self, String> {
        let players = session
            .roster
            .manager
            .players
            .iter()
            .map(|player| {
                let character = u32::from_le_bytes(player.bytes[0x150..0x154].try_into().unwrap());
                if character >= 10 {
                    return Err("Stats player character leaves original descriptor table".into());
                }
                let name = &player.bytes[0x158..0x178];
                let end = name
                    .iter()
                    .position(|&byte| byte == 0)
                    .ok_or("Stats player name lacks original32-byte terminator")?;
                Ok(Player {
                    character,
                    name: name[..end].to_vec(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        if players.len() > 6 || state.visible != std::array::from_fn(|slot| slot < players.len()) {
            return Err("Stats snapshot selectors disagree with actualmanager count".into());
        }
        Ok(Self {
            state: state.clone(),
            players,
            history: session.history.as_bytes().to_vec(),
            years_per_day: session.world.settings.years_per_day() as i32,
        })
    }
    pub fn graph(&self, bounds: [f32; 4]) -> Result<Graph, String> {
        Graph::from_history(
            &crate::player_history::State::from_bytes(&self.history)?,
            &self.state,
            bounds,
            self.years_per_day,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Graph {
    /// Native42ccc0/42d220 plot allocation through+64, retained as explicit words.
    pub words: [u32; 26],
    /// Native49e4d8 iterates manager slots backwards. Hidden selectors affect
    /// scale computation, but only selected slots produce these streams.
    pub series: Vec<(usize, Vec<[f32; 2]>)>,
}

pub struct Grid {
    pub rectangles: Vec<([f32; 4], u32)>,
    ///42ce80(x,y,value,decimal precision,format,alignment).
    pub labels: Vec<[u32; 6]>,
}
fn tick_step(raw: f32, count: u32, kind: u32) -> f32 {
    if !raw.is_finite() || raw == 0. {
        return raw;
    }
    let tenth = f64::from(0.1f32);
    if kind == 3 {
        let mut value = f64::from(raw.abs());
        let mut power = 1f64;
        while value < 1. {
            value *= 10.;
            power *= tenth;
        }
        while value > 10. {
            value *= tenth;
            power *= 10.;
        }
        return (value.trunc() * f64::from(power as f32) * f64::from(raw.signum())) as f32;
    }
    let mut value = f64::from((f64::from(count) * f64::from(raw) * 100.) as f32);
    let mut power = f64::from(0.01f32);
    while value > 100. {
        value *= tenth;
        power *= 10.;
    }
    let scale = if value < 8. {
        5.
    } else if value < 17. {
        10.
    } else if kind != 0 && value < 40. {
        20.
    } else if value < 75. {
        50.
    } else {
        100.
    };
    (power * scale / f64::from(count)) as f32
}
impl Graph {
    /// Native42d220 axis stream for49de60's48/32 spacing and supplied
    /// major/minor colors. Its tick index uses nearest-even FIST afterFST,
    /// while the first tick uses truncating FTOL before range admission.
    pub fn grid(&self) -> Result<Grid, String> {
        let f = |i| f32::from_bits(self.words[i]);
        let mut grid = Grid {
            rectangles: Vec::new(),
            labels: Vec::new(),
        };
        if f(10) == 0. || f(11) == 0. {
            return Ok(grid);
        }
        if self.words[14] != 2
            || self.words[15] != 2
            || !matches!(self.words[16], 0 | 3)
            || self.words[17] > 2
            || self.words[12] != 0
            || self.words[13] != 0
        {
            return Err("Stats grid leaves original initialized axis domain".into());
        }
        let step_x = tick_step(
            (f64::from(f(10)) / (f64::from(f(6)) / 48.)) as f32,
            2,
            self.words[16],
        );
        let step_y = tick_step((f64::from(f(11)) / (f64::from(f(7)) / 32.)) as f32, 2, 1);
        if [step_x, step_y]
            .into_iter()
            .any(|step| !step.is_finite() || step <= 0.)
        {
            return Err("Stats grid spacing is nonpositive or non-finite".into());
        }
        let end_x = (f64::from(f(8)) + f64::from(f(10))) as f32;
        let end_y = (f64::from(f(9)) + f64::from(f(11))) as f32;
        let scale_x = (f64::from(f(6)) / f64::from(f(10))) as f32;
        let scale_y = (f64::from(f(7)) / f64::from(f(11))) as f32;
        let first = |minimum: f32, step: f32| {
            // Native FTOL→FILD converts a fractional negative start to integer
            // zero, discarding its sign before multiplication by the step.
            let integer = (f64::from(minimum) / f64::from(step)).trunc() as i32;
            let mut tick = (f64::from(integer) * f64::from(step)) as f32;
            if tick < minimum && (f64::from(tick) - f64::from(minimum)).abs() > f64::from(0.001f32)
            {
                tick = (f64::from(tick) + f64::from(step)) as f32;
            }
            tick
        };
        let color = 0xff1f1f1f;
        let axis_color = 0xff3f3f3f;
        let mut tick = first(f(8), step_x);
        let mut iterations = 0;
        while tick < end_x {
            iterations += 1;
            if iterations > 4096 {
                return Err("Stats grid exceeds bounded native plot ticks".into());
            }
            let x = (f64::from(tick) - f64::from(f(8))) * f64::from(scale_x) + f64::from(f(4));
            let index = ((f64::from(tick) / f64::from(step_x)) as f32).round_ties_even() as i32;
            if index % 2 == 0 {
                grid.labels.push([
                    (x as f32).to_bits(),
                    ((f64::from(f(5)) + f64::from(f(7))) as f32).to_bits(),
                    ((f64::from(tick) * f64::from(f32::from_bits(0x3f800001))) as f32).to_bits(),
                    self.words[18],
                    self.words[16],
                    8,
                ]);
            }
            grid.rectangles.push((
                [
                    x as f32,
                    f(5),
                    (x + 1.) as f32,
                    (f64::from(f(5)) + f64::from(f(7))) as f32,
                ],
                color,
            ));
            tick = (f64::from(tick) + f64::from(step_x)) as f32;
        }
        let mut tick = first(f(9), step_y);
        iterations = 0;
        while tick < end_y {
            iterations += 1;
            if iterations > 4096 {
                return Err("Stats grid exceeds bounded native plot ticks".into());
            }
            let y = f64::from(f(7)) + f64::from(f(5))
                - (f64::from(tick) - f64::from(f(9))) * f64::from(scale_y);
            let index = ((f64::from(tick) / f64::from(step_y)) as f32).round_ties_even() as i32;
            if index % 2 == 0 {
                grid.labels.push([
                    f(4).to_bits(),
                    (y as f32).to_bits(),
                    ((f64::from(tick) * f64::from(f32::from_bits(0x3f800001))) as f32).to_bits(),
                    self.words[19],
                    self.words[17],
                    2,
                ]);
            }
            grid.rectangles.push((
                [
                    f(4),
                    y as f32,
                    (f64::from(f(6)) + f64::from(f(4))) as f32,
                    (y + 1.) as f32,
                ],
                color,
            ));
            tick = (f64::from(tick) + f64::from(step_y)) as f32;
        }
        if f(8) <= f(20) && end_x >= f(20) {
            let x = (f64::from(f(20)) - f64::from(f(8))) * f64::from(scale_x) + f64::from(f(4));
            grid.rectangles.push((
                [
                    (x - 1.) as f32,
                    f(5),
                    (x + 1.) as f32,
                    (f64::from(f(5)) + f64::from(f(7))) as f32,
                ],
                axis_color,
            ));
        }
        if f(9) <= f(21) && end_y >= f(21) {
            let y = f64::from(f(7)) + f64::from(f(5))
                - (f64::from(f(21)) - f64::from(f(9))) * f64::from(scale_y);
            grid.rectangles.push((
                [
                    f(4),
                    (y - 1.) as f32,
                    (f64::from(f(6)) + f64::from(f(4))) as f32,
                    (y + 1.) as f32,
                ],
                axis_color,
            ));
        }
        Ok(grid)
    }
}

///42d970 expands one history line. Native endpoint construction mirrors only
/// X, retaining its neighbor's Y; ordinary perpendicular caps are different.
pub fn line_quads(
    points: &[[f32; 2]],
    plot: [f32; 8],
    width: f32,
) -> Result<Vec<[[f32; 3]; 4]>, String> {
    if points.len() < 2 {
        return Ok(Vec::new());
    }
    if points.len() > 1024
        || plot.iter().chain([&width]).any(|v| !v.is_finite())
        || plot[6] == 0.
        || plot[7] == 0.
        || points.iter().flatten().any(|v| !v.is_finite())
    {
        return Err("Stats line leaves native finite plot domain".into());
    }
    let [
        left,
        top,
        plot_width,
        plot_height,
        min_x,
        min_y,
        range_x,
        range_y,
    ] = plot.map(f64::from);
    let project = |[x, y]: [f64; 2]| {
        [
            (x - min_x) * plot_width / range_x + left,
            plot_height + top - (y - min_y) * plot_height / range_y,
        ]
    };
    let mut edges = Vec::with_capacity(points.len());
    for (index, &point) in points.iter().enumerate() {
        let [x, y] = point.map(f64::from);
        let current = project([x, y]).map(|v| v as f32);
        let previous = if index == 0 {
            [2. * x - f64::from(points[1][0]), f64::from(points[1][1])]
        } else {
            points[index - 1].map(f64::from)
        };
        let next = if index + 1 == points.len() {
            [
                f64::from((2. * x - f64::from(points[index - 1][0])) as f32),
                f64::from(points[index - 1][1]),
            ]
        } else {
            points[index + 1].map(f64::from)
        };
        let a = project(previous);
        let b = project(next);
        let mut a = std::array::from_fn::<_, 2, _>(|i| a[i] - f64::from(current[i]));
        let mut b = std::array::from_fn::<_, 2, _>(|i| b[i] - f64::from(current[i]));
        let normalize = |vector: &mut [f64; 2]| {
            let length = (vector[0] * vector[0] + vector[1] * vector[1]).sqrt();
            if length != 0. {
                vector[0] /= length;
                vector[1] /= length;
            }
        };
        normalize(&mut a);
        normalize(&mut b);
        let mut miter = [-(b[1] - a[1]) as f32, (b[0] - a[0]) as f32];
        let length = (f64::from(miter[0]) * f64::from(miter[0])
            + f64::from(miter[1]) * f64::from(miter[1]))
        .sqrt();
        if length != 0. {
            miter = miter.map(|v| (f64::from(v) / length) as f32);
        }
        let dot = ((b[0] * a[0] + b[1] * a[1]) as f32).clamp(-1., 1.);
        let angle = if dot == -1. {
            f64::from(f32::from_bits(0x40490fdb))
        } else {
            f64::from(dot).acos()
        };
        let sine = (angle * 0.5).sin();
        let scale = if sine < f64::from(0.1f32) {
            10.
        } else {
            1. / sine
        } * f64::from(width)
            * 0.5;
        let dx = (scale * f64::from(miter[0])) as f32;
        let dy = scale * f64::from(miter[1]);
        let plus = [
            (f64::from(current[0]) + f64::from(dx)) as f32,
            (f64::from(current[1]) + dy) as f32,
            0.5,
        ];
        let minus = [
            (f64::from(current[0]) - f64::from(dx)) as f32,
            (f64::from(current[1]) - dy) as f32,
            0.5,
        ];
        edges.push([minus, plus]);
    }
    Ok((0..points.len() - 1)
        .rev()
        .map(|i| [edges[i][0], edges[i][1], edges[i + 1][1], edges[i + 1][0]])
        .collect())
}
impl Graph {
    pub fn from_history(
        history: &crate::player_history::State,
        state: &State,
        bounds: [f32; 4],
        years_per_day: i32,
    ) -> Result<Self, String> {
        let bytes = history.as_bytes();
        let int = |at| i32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        let float = |at| f32::from_bits(int(at) as u32);
        let mode = state.mode;
        if mode >= 6 {
            return Err("Stats metric leaves native six-series table".into());
        }
        let frequency = int(0x10f20 + mode * 4);
        if !(0..=1).contains(&frequency) {
            return Err("Stats history frequency leaves native initialized domains".into());
        }
        let hour = int(0x10f38);
        let year = int(0x10f3c)
            .wrapping_add(int(0x10f40).wrapping_mul(10))
            .wrapping_add(1930);
        let mut counts = [0usize; 6];
        for (slot, count) in counts.iter_mut().enumerate() {
            if state.visible[slot] {
                let n = int(0x10e90 + (slot * 6 + mode) * 4);
                if !(0..=480).contains(&n) {
                    return Err("Stats history count leaves retained480-sample ring".into());
                }
                *count = n as usize;
            }
        }
        let maximum = counts.iter().copied().max().unwrap_or(0);
        let mut words = [0u32; 26];
        let [left, top, right, bottom] = bounds;
        let width = right - left;
        let height = bottom - top;
        for (i, v) in [
            left,
            top,
            width,
            height,
            (f64::from(left) + 80.) as f32,
            (f64::from(top) + 5.) as f32,
            (f64::from(width) - 110.) as f32,
            (f64::from(height) - 35.) as f32,
        ]
        .into_iter()
        .enumerate()
        {
            words[i] = v.to_bits();
        }
        words[14] = 2;
        words[15] = 2;
        words[17] = match mode {
            1 | 2 | 4 => 1,
            5 => 2,
            _ => 0,
        };
        words[20] = 1930f32.to_bits();
        let mut min_x = f32::MAX;
        let mut max_x = -f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_y = -f32::MAX;
        let xs: Vec<f32> = (0..maximum)
            .map(|sample| {
                if frequency == 1 {
                    f64::from(year) + f64::from(years_per_day.wrapping_mul(sample as i32))
                } else if maximum > 48 {
                    f64::from(year)
                        + f64::from(years_per_day.wrapping_mul(hour.wrapping_add(sample as i32)))
                            * f64::from(f32::from_bits(0x3d2aaaab))
                } else {
                    f64::from(hour.wrapping_add(sample as i32))
                }
            })
            .map(|v| v as f32)
            .collect();
        if frequency == 0 {
            if maximum > 48 {
                min_x = min_x.min(year as f32);
                max_x = max_x.max(year as f32);
            } else {
                words[16] = 3;
                words[20] = (hour as f32).to_bits();
                let end = hour.wrapping_add(24) as f32;
                min_x = min_x.min(end);
                max_x = max_x.max(end);
            }
        }
        let mut streams = Vec::new();
        for slot in (0..6).rev().filter(|&slot| state.visible[slot]) {
            let cursor = int(0x10e00 + (slot * 6 + mode) * 4);
            if !(0..480).contains(&cursor) {
                return Err("Stats history cursor leaves retained ring".into());
            }
            let count = counts[slot];
            let mut points = Vec::with_capacity(count);
            for (sample, &x) in xs.iter().take(count).enumerate() {
                let index = (cursor + sample as i32 - count as i32 + 480) % 480;
                let y = float(((slot * 6 + mode) * 480 + index as usize) * 4);
                if !x.is_finite() || !y.is_finite() {
                    return Err("Stats history graph contains non-finite sample".into());
                }
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                points.push([x, y]);
            }
            if state.selected[slot] {
                streams.push((slot, points));
            }
        }
        max_y = max_y.max(0.);
        min_y = min_y.min(0.);
        words[22] = min_x.to_bits();
        words[23] = min_y.to_bits();
        words[24] = max_x.to_bits();
        words[25] = max_y.to_bits();
        //42ccc0 keeps PC64 lower margin but stores the upper margin to F32.
        // A zero Y range retains its original +10/-1 asymmetric defaults.
        let delta = f64::from(max_y) - f64::from(min_y);
        let margin = if delta == 0. {
            1.
        } else {
            f64::from(delta as f32) * 0.25
        };
        let upper = if delta == 0. { 10f32 } else { margin as f32 };
        let high = (f64::from(max_y) + f64::from(upper)) as f32;
        let low = (f64::from(min_y) - margin) as f32;
        if max_x > min_x && high > low {
            words[8] = min_x.to_bits();
            words[9] = low.to_bits();
            words[10] = ((f64::from(max_x) - f64::from(min_x)) as f32).to_bits();
            words[11] = ((f64::from(high) - f64::from(low)) as f32).to_bits();
        }
        Ok(Self {
            words,
            series: streams,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// A statistics-panel button action (original 49e630).
pub enum Action {
    Open,
    Close,
    Cycle(bool),
    Toggle(usize),
}
