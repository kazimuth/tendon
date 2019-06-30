use crate::Path;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Trait {
    pub path: Path,
}
