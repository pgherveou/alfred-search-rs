//! Data representation for Alfred result items
// See [specifications](https://www.alfredapp.com/help/workflows/inputs/script-filter/json/)
use serde::Serialize;

#[derive(Serialize, Default)]
pub struct AlfredItem {
    /// The title displayed in the result row
    pub title: String,
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//
//     #[test]
//     fn serialize_item() {
//         println!("hello from test");
//     }
// }
