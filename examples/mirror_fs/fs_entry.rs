use std::collections::BTreeSet;

use intaglio::Symbol;
use nfs_mamont::xdr::nfs3::{FAttr3, FType3, FileId3};

/// A file system entry representing a file or directory
#[derive(Debug, Clone)]
pub struct FSEntry {
    /// The name of the entry as a list of symbols
    pub name: Vec<Symbol>,
    /// The file attributes of the entry
    pub fsmeta: FAttr3,
    /// Metadata when building the children list
    pub children_meta: FAttr3,
    /// Optional set of child file IDs
    pub children: Option<BTreeSet<FileId3>>,
}

impl FSEntry {
    /// Creates a new file system entry
    pub fn new(name: Vec<Symbol>, fsmeta: FAttr3) -> Self {
        Self { name, fsmeta, children_meta: fsmeta, children: None }
    }

    /// Checks if the entry is a directory
    pub fn is_directory(&self) -> bool {
        matches!(self.fsmeta.ftype, FType3::NF3DIR)
    }

    /// Checks if the entry has children
    pub fn has_children(&self) -> bool {
        self.children.is_some()
    }

    /// Adds a child to the entry
    pub fn add_child(&mut self, child_id: FileId3) {
        if let Some(ref mut children) = self.children {
            children.insert(child_id);
        } else {
            self.children = Some(BTreeSet::from([child_id]));
        }
    }

    /// Removes a child from the entry
    pub fn remove_child(&mut self, child_id: FileId3) {
        if let Some(ref mut children) = self.children {
            children.remove(&child_id);
        }
    }
}
