//! The reserved-id mint [SPEC 21/23]: `lini-<what>-N`, 1-based in statement
//! order, skipping names already taken — so a re-desugared scope that gained a
//! capsule or a label wire never collides with the ids the last pass minted.
//!
//! One counter, one taken set, one spelling — the capsule hoist and the
//! label-wire mint differ only in their prefix.

use std::collections::HashSet;

pub(super) struct Mint {
    what: &'static str,
    taken: HashSet<String>,
    next: usize,
}

impl Mint {
    /// A mint over `lini-<what>-N`, seeded with the scope's declared names.
    pub(super) fn new(what: &'static str, declared: &HashSet<String>) -> Self {
        Self {
            what,
            taken: declared.clone(),
            next: 1,
        }
    }

    /// The next free reserved id, reserved as it is handed out.
    pub(super) fn next_id(&mut self) -> String {
        let mut id = self.name();
        while self.taken.contains(&id) {
            self.next += 1;
            id = self.name();
        }
        self.next += 1;
        self.taken.insert(id.clone());
        id
    }

    /// Take a name out of circulation — an authored id the mint must skip.
    pub(super) fn reserve(&mut self, id: String) {
        self.taken.insert(id);
    }

    fn name(&self) -> String {
        format!("lini-{}-{}", self.what, self.next)
    }
}
