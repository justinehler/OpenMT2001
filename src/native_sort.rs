//! Original50fab1 CRT quicksort and small-array selection sort. Equal keys
//! retain the original swap order; this is deliberately not a stable sort.
pub fn sort<T: Copy>(rows: &mut [T], compare: &impl Fn(T, T) -> i32) {
    if rows.len() < 2 {
        return;
    }
    if rows.len() <= 8 {
        for end in (1..rows.len()).rev() {
            let mut greatest = 0;
            for i in 1..=end {
                if compare(rows[i], rows[greatest]) > 0 {
                    greatest = i;
                }
            }
            rows.swap(greatest, end);
        }
        return;
    }
    rows.swap(rows.len() / 2, 0);
    let mut i = 0;
    let mut j = rows.len();
    loop {
        i += 1;
        while i < rows.len() && compare(rows[i], rows[0]) <= 0 {
            i += 1;
        }
        j -= 1;
        while j > 0 && compare(rows[j], rows[0]) >= 0 {
            j -= 1;
        }
        if j < i {
            break;
        }
        rows.swap(i, j);
    }
    rows.swap(0, j);
    if j > 1 {
        sort(&mut rows[..j], compare);
    }
    if i + 1 < rows.len() {
        sort(&mut rows[i..], compare);
    }
}
