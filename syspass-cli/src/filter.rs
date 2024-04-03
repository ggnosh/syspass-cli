use std::fmt::Display;

pub fn filter<T: Display>(input: &str, _option: &T, string_value: &str, _idx: usize) -> Option<i64> {
    let filter = input.to_lowercase();
    if string_value.to_lowercase().contains(&filter) {
        Some(0)
    } else {
        None
    }
}
