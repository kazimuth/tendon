use std::cell::RefCell;
use std::rc::{Rc, Weak};
use tendon_api::paths::Ident;
use tendon_api::items::DeclarativeMacroItem;

/// A textual scope -- the old, ordered way macros were resolved.
///
/// We can't just do these all in one pass, because the following code compiles:
/// ```
/// in_t!();
/// c!();
///
/// mod t {
///     #[macro_export]
///     macro_rules! in_t {
///         () => {
///             macro_rules! c {
///                 () => ();
///             }
///         };
///     }
/// }
/// ```
/// So the invocation of `c!()` needs a pointer to *some* scope before
/// that scope actually contains the relevant macro. Mind-bending.
///
/// Also, scopes need to be nestable:
/// ```
/// in_t!();
/// e!();
///
/// mod t {
///     #[macro_export]
///     macro_rules! in_t {
///         () => {
///             macro_rules! c {
///                 () => ();
///             }
///             macro_rules! e {
///                 () => (c!());
///             }
///         };
///     }
/// }
/// ```
///
/// Each UnexpandedItem has a TextualScope, which contains all the names which might
/// end up being textually resolved to that item.
///
/// At the leading edge of a module, we have a scope. We want to be able to add new definitions to it.
/// Also, we want to be able to add to a leading edge *within* a module, during expansion.
///
/// Expanding an item should not modify that item's scope, but it should modify all the scopes that
/// come after it.
///
/// Alright, let's bite the bullet and doubly-linked-list it.
#[derive(Clone)]
pub(crate) struct TextualScope(Rc<RefCell<TextualScopeInner>>);

impl TextualScope {
    /// Create an empty scope.
    pub(crate) fn empty() -> TextualScope {
        TextualScope(Rc::new(RefCell::new(TextualScopeInner {
            previous: None,
            definition: None,
            next: None,
        })))
    }

    /// Lookup a name within this scope.
    pub(crate) fn lookup(&self, name: &Ident) -> Option<Rc<DeclarativeMacroItem>> {
        let self_ = self.0.borrow();
        if let Some(definition) = self_.definition.as_ref() {
            if &definition.name == name {
                return Some(definition.clone());
            }
        }
        if let Some(previous) = self_.previous.as_ref() {
            return previous.lookup(name);
        }
        None
    }

    /// Append a definition to this scope, returning a new scope containing that definition.
    pub(crate) fn append_scope(&self, definition: Option<DeclarativeMacroItem>) -> TextualScope {
        let self_clone = self.clone();
        let mut self_ = self.0.borrow_mut();
        let definition = definition.map(Rc::new);

        if let Some(Some(next)) = self_.next.as_ref().map(|weak| weak.upgrade()) {
            // build the child with outward links
            let child = TextualScope(Rc::new(RefCell::new(TextualScopeInner {
                previous: Some(self_clone),
                next: Some(Rc::downgrade(&next)),
                definition,
            })));
            // set back link from next
            next.borrow_mut().previous = Some(child.clone());
            // set forward link from us
            self_.next = Some(Rc::downgrade(&child.0));
            // return the child
            child
        } else {
            // set the child with a backward lin
            let child = TextualScope(Rc::new(RefCell::new(TextualScopeInner {
                previous: Some(self_clone),
                next: None,
                definition,
            })));
            // set our forward link
            self_.next = Some(Rc::downgrade(&child.0));
            // return the child
            child
        }
    }

    /// Create a submodule that inherits from this scope, but does
    /// not affect later entries in this scope.
    pub(crate) fn make_dead_submodule(&self) -> TextualScope {
        // we sneakily insert a tracker scope here, so that changes after the last scope before
        // the submodule still propagate
        let tracker = self.append_scope(None);
        // and then make a scope without forwarding that points to the tracker
        TextualScope(Rc::new(RefCell::new(TextualScopeInner {
            previous: Some(tracker),
            next: None,
            definition: None,
        })))
    }
}

struct TextualScopeInner {
    previous: Option<TextualScope>,
    definition: Option<Rc<DeclarativeMacroItem>>,
    next: Option<Weak<RefCell<TextualScopeInner>>>,
}

#[cfg(test)]
mod tests {
    use super::TextualScope;
    use tendon_api::attributes::Metadata;
    use tendon_api::items::DeclarativeMacroItem;
    use tendon_api::tokens::Tokens;

    // The first test emulates the following code:
    // (inlined so we can be sure it works.)
    macro_rules! a {
        () => {};
    }
    crate::b!(); // defines c and e
    e!(); // calls c, which defines d
    d!();
    a!();
    mod t {
        #[macro_export]
        macro_rules! b {
            () => {
                macro_rules! c {
                    () => {
                        macro_rules! d {
                            () => {};
                        }
                    };
                }
                macro_rules! e {
                    () => {
                        c!();
                    };
                }
            };
        }
    }

    #[allow(unused)]
    #[test]
    fn hairy() {
        let root = TextualScope::empty();
        let a = root.append_scope(Some(fake_macro("a")));
        let mut b_call = a.append_scope(None);
        let e_call = b_call.append_scope(None);
        let d_call = e_call.append_scope(None);
        let a_call = d_call.append_scope(None);

        // a should be defined everywhere but root
        assert!(a_call.lookup(&"a".into()).is_some());
        assert!(root.lookup(&"a".into()).is_none());

        // expand b_call
        let c = b_call.append_scope(Some(fake_macro("c")));
        let e = c.append_scope(Some(fake_macro("e")));
        b_call = e.clone();

        // new macros should not be visible before
        assert!(a.lookup(&"c".into()).is_none());
        assert!(a.lookup(&"e".into()).is_none());
        // new macros should be visible (anywhere) after
        assert!(b_call.lookup(&"c".into()).is_some());
        assert!(a_call.lookup(&"c".into()).is_some());

        // expand e_call
        let mut c_call = e_call.append_scope(None);
        let d = c_call.append_scope(Some(fake_macro("d")));
        c_call = d.clone();

        // before e_call, d should not be defined
        assert!(b_call.lookup(&"d".into()).is_none());
        // after, it should
        assert!(d_call.lookup(&"d".into()).is_some());

        // a should still be defined everywhere
        assert!(a_call.lookup(&"a".into()).is_some());
        assert!(root.lookup(&"a".into()).is_none());
    }

    #[test]
    fn submodules() {
        let root = TextualScope::empty();
        let defn = root.append_scope(Some(fake_macro("defn")));
        let mut expands_to_defn = defn.append_scope(None);
        let dead_submodule = expands_to_defn.make_dead_submodule();
        let afterwards = expands_to_defn.append_scope(None);

        assert!(expands_to_defn.lookup(&"defn".into()).is_some());
        assert!(dead_submodule.lookup(&"defn".into()).is_some());
        assert!(afterwards.lookup(&"defn".into()).is_some());

        expands_to_defn = expands_to_defn.append_scope(Some(fake_macro("expanded")));

        assert!(expands_to_defn.lookup(&"expanded".into()).is_some());
        assert!(dead_submodule.lookup(&"expanded".into()).is_some());
        assert!(afterwards.lookup(&"expanded".into()).is_some());

        let dead_defn = dead_submodule.append_scope(Some(fake_macro("dead")));
        // should be visible after definition
        assert!(dead_defn.lookup(&"dead".into()).is_some());
        // but not before
        assert!(expands_to_defn.lookup(&"dead".into()).is_none());
        // and not outside
        assert!(afterwards.lookup(&"dead".into()).is_none());
    }

    fn fake_macro(name: &str) -> DeclarativeMacroItem {
        // for this test, nothing but the name matters.
        DeclarativeMacroItem {
            metadata: Metadata::fake(),
            macro_export: false,
            name: name.into(),
            tokens: Tokens::from(""),
        }
    }
}
