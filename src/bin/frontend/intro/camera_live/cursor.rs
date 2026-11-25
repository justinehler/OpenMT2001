//! Finite scalar part of 4391e0. All real UI/camera requesters share this value.
//! The existing host owns original images/hotspots; this is not an image queue.
use openmt2001::camera::primary::CursorRequest;

#[derive(Clone, Copy)]
pub struct Cursor { state: u32, priority: i32 }

impl Cursor {
    /// Supply actual process state; neither a save nor camera mode supplies it.
    pub fn new(state: u32, priority: i32) -> Result<Self, String> {
        host_index(state)?;
        Ok(Self { state, priority })
    }
    pub fn select(&mut self, mut request: CursorRequest, force_default: bool) -> Result<(), String> {
        // Original 8b22d8 && 8b22dc && !86d675 override precedes priority.
        if force_default { request.state = 0; }
        if request.priority < self.priority { return Ok(()); }
        host_index(request.state)?;
        self.priority = request.priority;
        self.state = request.state; // equality accepted: later request wins
        Ok(())
    }
    /// Original presentation369 lower_priority: once at the real presentation
    /// boundary, not once per camera catchup or arbitrary cursor query.
    pub fn presented(&mut self) { self.priority = self.priority.wrapping_sub(1); }
    pub fn host_index(&self) -> usize {
        // Constructor/select admit exactly the four installed host images.
        match self.state { 0 => 0, 1 => 3, 2 => 1, 3 => 2, _ => unreachable!() }
    }
}

fn host_index(state: u32) -> Result<usize, String> {
    match state {
        0 => Ok(0), 1 => Ok(3), 2 => Ok(1), 3 => Ok(2),
        _ => Err(format!("Native cursor {state} has no installed host image")),
    }
}
