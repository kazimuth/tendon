/// Name resolution and macro expansion.
/// Works asyncronously and memoizes as it goes in order to achieve MAXIMUM HARDWARE EXPLOITATION.

use cargo_metadata::PackageId;

pub mod item_expand;

pub struct ResolvedPath {
    /// The crate instantiation this path comes from.
    pub package: PackageId,
    /// The path of the item, rooted within that package.
    pub path: Vec<String>,
}

/// A resolver within the context of a particular module. Tracks all imports, definitions etc. within that module.
struct ModuleCtx {}
impl ModuleCtx {
    async fn resolve(&self, path: &syn::Path) -> Result<ResolvedPath> {
        unimplemented!();
    }
}

/// Get the source for some module.
async fn get_module(path: &ResolvedPath) -> Resulve<(Vec<syn::Attribute>, Vec<syn::Item>)> {
    unimplemented!()
}

/// A scraped type.
struct Type {}

/// A scraped trait.
struct Trait {}

/// A scraped const.
struct Const {}

/// A scraped static.
struct Static {}