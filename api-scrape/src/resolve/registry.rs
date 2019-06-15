
//! Lookup... from a registry.
//! 
//! Note from cargo:
//! 
//!
//! ```notrust
//! # A folder under which all registry metadata is hosted (similar to
//! # $HOME/.cargo/git)
//! $HOME/.cargo/registry/
//!
//!     # For each registry that cargo knows about (keyed by hostname + hash)
//!     # there is a folder which is the checked out version of the index for
//!     # the registry in this location. Note that this is done so cargo can
//!     # support multiple registries simultaneously
//!     index/
//!         registry1-<hash>/
//!         registry2-<hash>/
//!         ...
//!
//!     # This folder is a cache for all downloaded tarballs from a registry.
//!     # Once downloaded and verified, a tarball never changes.
//!     cache/
//!         registry1-<hash>/<pkg>-<version>.crate
//!         ...
//!
//!     # Location in which all tarballs are unpacked. Each tarball is known to
//!     # be frozen after downloading, so transitively this folder is also
//!     # frozen once its unpacked (it's never unpacked again)
//!     src/
//!         registry1-<hash>/<pkg>-<version>/...
//!         ...
//! ```