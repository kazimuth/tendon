
pub mod resolvable;

/*
use crate::{Error, Result};
use cargo_metadata::{CargoOpt, Metadata, Node, Package, PackageId};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use syn;
use quote::ToTokens;
use tracing::{info, info_span};
use transgress_api::{Ident};

mod namespace;
mod module;

/// The Resolver, the core data structure of this crate.
pub struct Resolver {
}
// do we want floating MethodItems? they... sorta have canonical paths? kinda same problem as traits
// -> attach extra scope information to some things

// macro name resolution is affected by order, right?
// -> a macro's meaning can't change by adding new rules, because if something would have matched before,
//    it'll still match after
// is it possible to view a macro's rules as having been declared in a different order?
// https://rust-lang.github.io/rustc-guide/name-resolution.html
// https://github.com/rust-lang/rust/blob/master/src/librustc_resolve/lib.rs
//
// TODO: group macros together per-module
// will have some weird edge cases... whatever
//
// note: don't type uses, allow passthrough (actually the better choice neway)


impl Resolver {
    pub fn new(project_root: PathBuf) -> Result<Resolver> {
        let db = Db::new();

        Ok(Resolver {
            packages,
            root,
            resolve,
            project_root,
            workspace_root,
        })
    }

    pub fn parse_crate(&self, id: PackageId) -> Result<()> {
        // look up in resolve?
        let span = info_span!("parse_crate", crate_ = &id.repr.split(" ").next().unwrap());
        let _entered = span.enter();

        info!("parsing crate {}", id.repr);
        let package = &self.packages[&id];
        let lib_target = &package
            .targets
            .iter()
            .find(|t| t.kind.iter().find(|k| *k == "lib").is_some())
            .ok_or(Error::Other {
                cause: "no lib target in crate",
            })?;
        let root = lib_target.src_path.clone();
        self.parse_module(root, ResolvedPath::new(&id, ""))
    }

    pub fn parse_module(&self, root: PathBuf, path: ResolvedPath) -> Result<()> {
        info!("parsing {} (`{}`)", pretty(&path), root.display());

        let mut file = File::open(root)?;
        let mut source = String::new();
        file.read_to_string(&mut source)?;

        let source = syn::parse_str::<syn::File>(&source)?;

        for item in &source.items {
            match item {
                syn::Item::Use(use_) => {
                    info!("use {}", use_.tree.clone().into_token_stream());
                }
                syn::Item::ExternCrate(crate_) => self.skip("crate", path.join(&crate_.ident)),
                syn::Item::Static(static_) => self.skip("static", path.join(&static_.ident)),
                syn::Item::Const(const_) => self.skip("const", path.join(&const_.ident)),
                syn::Item::Fn(fn_) => self.skip("fn", path.join(&fn_.ident)),
                syn::Item::Mod(mod_) => self.skip("mod", path.join(&mod_.ident)),
                syn::Item::Type(type_) => self.skip("type", path.join(&type_.ident)),
                syn::Item::Existential(existential_) => {
                    self.skip("existential", path.join(&existential_.ident))
                }
                syn::Item::Struct(struct_) => self.skip("struct", path.join(&struct_.ident))
                syn::Item::Enum(enum_) => self.skip("enum", path.join(&enum_.ident)),
                syn::Item::Union(union_) => self.skip("union", path.join(&union_.ident)),
                syn::Item::Trait(trait_) => self.skip("trait", path.join(&trait_.ident)),
                syn::Item::TraitAlias(alias_) => self.skip("alias", path.join(&alias_.ident)),
                syn::Item::Impl(impl_) => {
                    info!("impl: {}", impl_.into_token_stream());
                }
                syn::Item::Macro(macro_rules_) => {
                    if let Some(ident) = &macro_rules_.ident {
                        self.skip("macro_rules", path.join(ident))
                    }
                }
                syn::Item::Macro2(macro2_) => self.skip("macro2_", path.join(&macro2_.ident)),
                syn::Item::ForeignMod(_foreign_mod_) => self.skip("foreign_mod", path.clone()),
                syn::Item::Verbatim(_verbatim_) => self.skip("verbatim", path.clone()),
            }
        }

        // scope to target crate?

        // https://doc.rust-lang.org/stable/reference/items/modules.html
        // mod q; -> q/mod.rs; q.rs
        //
        // #[path="z.rs"] mod q -> z.rs
        // #[path="bees"] mod wasps { mod queen; } -> bees/queen.rs, bees/queen/mod.rs

        // cfg-attrs

        // prelude

        // check edition
        Ok(())
    }

    pub fn skip(&self, kind: &str, path: ResolvedPath) {
        info!("skipping {} {}", kind, pretty(&path));
    }
}

*/
