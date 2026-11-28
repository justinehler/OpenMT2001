//! Expand unified postings when comparing callbacks with original UI traces.
pub fn cash_calls(amount: f32, local: bool, threshold: f32) -> Vec<(u32, Vec<u32>)> {
    if !local {
        return Vec::new();
    }
    let credit = if amount > threshold {
        true
    } else if amount < threshold || amount.is_nan() {
        false
    } else {
        return Vec::new();
    };
    let sound = (
        0x4ad760,
        vec![if credit { 0x541cc8 } else { 0x541cb4 }, 0, 0, 0, 1, 60],
    );
    let pulse = (if credit { 0x4c6290 } else { 0x4c62d0 }, vec![]);
    if credit {
        vec![sound, pulse]
    } else {
        vec![pulse, sound]
    }
}
