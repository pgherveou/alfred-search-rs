//! Data representation for Alfred result items
// See [specifications](https://www.alfredapp.com/help/workflows/inputs/script-filter/json/)
use serde::Serialize;

use crate::{crate_client::CrateSearchItem, gh_client::GHApiRepoSearchItem};

#[derive(Serialize, Default)]
pub struct AlfredItem {
    /// The title displayed in the result row
    pub title: String,
}

impl From<String> for AlfredItem {
    fn from(val: String) -> Self {
        Self { title: val }
    }
}

impl From<GHApiRepoSearchItem> for AlfredItem {
    fn from(val: GHApiRepoSearchItem) -> Self {
        Self {
            title: val.full_name,
        }
    }
}
impl From<CrateSearchItem> for AlfredItem {
    fn from(value: CrateSearchItem) -> Self {
        Self { title: value.name }
    }
}
