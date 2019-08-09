use transgress_api::{Ident, Path};
use std::path::PathBuf as FsPathBuf;
use crate::Map;

/// A possibly-resolved module.
#[derive(Debug)]
pub enum Module {
    /// A module that has not yet been parsed.
    /// Note that modules are only parsed as-needed!
    Unparsed {
        /// Where this module can be found in the filesystem.
        path: FsPathBuf,
    },
    /// A parsed module.
    Parsed {
        /// This module's glob imports.
        /// `use x::y::z::*` is stored as `x::y::z` pre-resolution,
        /// and as an AbsolutePath post-resolution.
        /// Includes the prelude, if any.
        /// These aren't guaranteed to be resolved! We resolve as we go :)
        glob_imports: Vec<Path>,
        /// This module's non-glob imports.
        /// Maps the imported-as ident to a path,
        /// i.e. `use x::Y;` is stored as `Y => x::Y`,
        /// `use x::z as w` is stored as `w => x::z`
        imports: Map<Ident, Path>,
        // TODO #[macro_use] imports
        // TODO unresolved macros; needs Send pm2 shim
    }
}
