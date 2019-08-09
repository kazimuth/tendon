//! Namespaces.
use transgress_api::paths::AbsolutePath;
use crate::Map;

/// A namespace, for holding some particular type of item during resolution.
pub(crate) struct Namespace<I> {
    map: Map<AbsolutePath, I>
}

