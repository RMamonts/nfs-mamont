//! Directory procedures: name resolution, listing and (forgotten) mutations.

use nfs_mamont::auth::Credential;
use nfs_mamont::consts::nfsv3::NFS3_FHSIZE;
use nfs_mamont::consts::xdr::ALIGNMENT;
use nfs_mamont::vfs::file::{Handle, Name};
use nfs_mamont::vfs::read_dir::{Cookie, CookieVerifier};
use nfs_mamont::vfs::{
    self, create, link, lookup, mk_dir, mk_node, read_dir, read_dir_plus, remove, rename, rm_dir,
    symlink,
};

use super::MockVfs;
use crate::tree::{Entry, Kind, ObjectId};

/// XDR size of `fattr3`.
const FATTR3_SIZE: usize = 84;
/// XDR overhead of a `READDIR3resok`/`READDIRPLUS3resok` besides its entries:
/// status (4), `dir_attributes` (4 + 84), `cookieverf` (8), list terminator (4), `eof` (4).
const REPLY_OVERHEAD: usize = 4 + 4 + FATTR3_SIZE + 8 + 4 + 4;
/// XDR size of an `entry3` without the name bytes:
/// `value_follows` (4), `fileid` (8), name length (4), `cookie` (8).
const ENTRY_OVERHEAD: usize = 4 + 8 + 4 + 8;
/// Extra XDR size of an `entryplus3` over an `entry3`:
/// `name_attributes` (4 + 84), `name_handle` (4 + 4 + padded handle bytes).
const ENTRY_PLUS_EXTRA: usize = 4 + FATTR3_SIZE + 4 + 4 + padded(NFS3_FHSIZE);

/// Rounds `len` up to the XDR alignment.
const fn padded(len: usize) -> usize {
    len.div_ceil(ALIGNMENT) * ALIGNMENT
}

/// Returns the directory behind `handle`, or [`vfs::Error::NotDir`].
fn dir_of(handle: &Handle) -> Result<ObjectId, vfs::Error> {
    let id = ObjectId::from_handle(handle);
    match id.kind() {
        Kind::Directory => Ok(id),
        Kind::Regular => Err(vfs::Error::NotDir),
    }
}

fn is_dot(name: &str) -> bool {
    name == "." || name == ".."
}

/// Builds a [`Name`] for a generated entry; the names are always valid, so a failure is
/// a server bug reported as [`vfs::Error::ServerFault`].
fn entry_name(name: String) -> Result<Name, vfs::Error> {
    Name::new(name).map_err(|_| vfs::Error::ServerFault)
}

impl MockVfs {
    /// Collects the entries of `dir` starting at cookie `start` that fit into the
    /// reply budgets.
    ///
    /// `dir_budget` bounds the `entry3` part of the entries (`dircount` of
    /// `READDIRPLUS`, the whole budget for `READDIR`), `total_budget` bounds the entries
    /// including `extra` bytes of attributes and handle per entry (`count`/`maxcount`
    /// without the fixed reply overhead).
    ///
    /// # Returns
    ///
    /// The entries and whether the end of the directory was reached.
    fn list(
        &self,
        dir: ObjectId,
        start: u64,
        dir_budget: usize,
        total_budget: usize,
        extra: usize,
    ) -> Result<(Vec<Entry>, bool), vfs::Error> {
        if start > self.layout.len() {
            return Err(vfs::Error::BadCookie);
        }

        let mut entries = Vec::new();
        let mut dir_used = 0;
        let mut total_used = 0;
        let mut eof = true;
        for entry in self.layout.entries(dir, start) {
            let dir_size = ENTRY_OVERHEAD + padded(entry.name.len());
            if dir_used + dir_size > dir_budget || total_used + dir_size + extra > total_budget {
                eof = false;
                break;
            }
            dir_used += dir_size;
            total_used += dir_size + extra;
            entries.push(entry);
        }

        if entries.is_empty() && !eof {
            return Err(vfs::Error::TooSmall);
        }

        Ok((entries, eof))
    }
}

impl lookup::Lookup for MockVfs {
    /// Resolves any name: see the naming rule in [`crate::tree`].
    async fn lookup(
        &self,
        args: lookup::Args,
        _cred: &Credential,
    ) -> Result<lookup::Success, lookup::Fail> {
        self.wait(0).await;
        let dir_attr = Some(self.attr_of(&args.parent));
        let dir = dir_of(&args.parent).map_err(|error| lookup::Fail { error, dir_attr })?;
        let child = dir.child(args.name.as_str());

        Ok(lookup::Success {
            file: child.handle(args.parent.backend_id()),
            file_attr: Some(self.attr(child)),
            dir_attr: Some(self.attr(dir)),
        })
    }
}

impl create::Create for MockVfs {
    /// Succeeds for names that denote regular files; every name already "exists", so
    /// guarded and exclusive creates fail with [`vfs::Error::Exist`].
    async fn create(
        &self,
        args: create::Args,
        _cred: &Credential,
    ) -> Result<create::Success, create::Fail> {
        self.wait(0).await;
        let wcc_data = self.wcc(ObjectId::from_handle(&args.object.dir));
        let fail = |error| create::Fail { error, wcc_data: wcc_data.clone() };

        let dir = dir_of(&args.object.dir).map_err(fail)?;
        let name = args.object.name.as_str();
        if is_dot(name) {
            return Err(fail(vfs::Error::Exist));
        }
        let child = dir.child(name);
        if child.kind() != Kind::Regular {
            return Err(fail(vfs::Error::IsDir));
        }

        match args.how {
            create::How::Unchecked(_) => Ok(create::Success {
                file: Some(child.handle(args.object.dir.backend_id())),
                attr: Some(self.attr(child)),
                wcc_data: self.wcc(dir),
            }),
            create::How::Guarded(_) | create::How::Exclusive(_) => Err(fail(vfs::Error::Exist)),
        }
    }
}

impl mk_dir::MkDir for MockVfs {
    /// Succeeds for names that denote directories (`dir*`); any other name is already
    /// taken by a regular file.
    async fn mk_dir(
        &self,
        args: mk_dir::Args,
        _cred: &Credential,
    ) -> Result<mk_dir::Success, mk_dir::Fail> {
        self.wait(0).await;
        let dir_wcc = self.wcc(ObjectId::from_handle(&args.object.dir));
        let fail = |error| mk_dir::Fail { error, dir_wcc: dir_wcc.clone() };

        let dir = dir_of(&args.object.dir).map_err(fail)?;
        let name = args.object.name.as_str();
        if is_dot(name) {
            return Err(fail(vfs::Error::Exist));
        }
        let child = dir.child(name);
        if child.kind() != Kind::Directory {
            return Err(fail(vfs::Error::Exist));
        }

        Ok(mk_dir::Success {
            file: Some(child.handle(args.object.dir.backend_id())),
            attr: Some(self.attr(child)),
            wcc_data: self.wcc(dir),
        })
    }
}

impl symlink::Symlink for MockVfs {
    async fn symlink(
        &self,
        args: symlink::Args,
        _cred: &Credential,
    ) -> Result<symlink::Success, symlink::Fail> {
        self.wait(0).await;

        Err(symlink::Fail {
            error: vfs::Error::NotSupported,
            dir_wcc: self.wcc(ObjectId::from_handle(&args.object.dir)),
        })
    }
}

impl mk_node::MkNode for MockVfs {
    async fn mk_node(
        &self,
        args: mk_node::Args,
        _cred: &Credential,
    ) -> Result<mk_node::Success, mk_node::Fail> {
        self.wait(0).await;

        Err(mk_node::Fail {
            error: vfs::Error::NotSupported,
            dir_wcc: self.wcc(ObjectId::from_handle(&args.object.dir)),
        })
    }
}

impl link::Link for MockVfs {
    async fn link(
        &self,
        args: link::Args,
        _cred: &Credential,
    ) -> Result<link::Success, link::Fail> {
        self.wait(0).await;

        Err(link::Fail {
            error: vfs::Error::NotSupported,
            file_attr: Some(self.attr_of(&args.file)),
            dir_wcc: self.wcc(ObjectId::from_handle(&args.link.dir)),
        })
    }
}

impl remove::Remove for MockVfs {
    /// Accepts the removal of regular files and forgets it.
    async fn remove(
        &self,
        args: remove::Args,
        _cred: &Credential,
    ) -> Result<remove::Success, remove::Fail> {
        self.wait(0).await;
        let dir_wcc = self.wcc(ObjectId::from_handle(&args.object.dir));
        let fail = |error| remove::Fail { error, dir_wcc: dir_wcc.clone() };

        let dir = dir_of(&args.object.dir).map_err(fail)?;
        let name = args.object.name.as_str();
        if is_dot(name) {
            return Err(fail(vfs::Error::InvalidArgument));
        }
        if dir.child(name).kind() != Kind::Regular {
            return Err(fail(vfs::Error::IsDir));
        }

        Ok(remove::Success { wcc_data: self.wcc(dir) })
    }
}

impl rm_dir::RmDir for MockVfs {
    /// Accepts the removal of directories and forgets it.
    async fn rm_dir(
        &self,
        args: rm_dir::Args,
        _cred: &Credential,
    ) -> Result<rm_dir::Success, rm_dir::Fail> {
        self.wait(0).await;
        let dir_wcc = self.wcc(ObjectId::from_handle(&args.object.dir));
        let fail = |error| rm_dir::Fail { error, dir_wcc: dir_wcc.clone() };

        let dir = dir_of(&args.object.dir).map_err(fail)?;
        let name = args.object.name.as_str();
        if name == "." {
            return Err(fail(vfs::Error::InvalidArgument));
        }
        if name == ".." {
            return Err(fail(vfs::Error::Exist));
        }
        if dir.child(name).kind() != Kind::Directory {
            return Err(fail(vfs::Error::NotDir));
        }

        Ok(rm_dir::Success { wcc_data: self.wcc(dir) })
    }
}

impl rename::Rename for MockVfs {
    /// Accepts renames between objects of the same kind and forgets them.
    async fn rename(
        &self,
        args: rename::Args,
        _cred: &Credential,
    ) -> Result<rename::Success, rename::Fail> {
        self.wait(0).await;
        let from_dir_wcc = self.wcc(ObjectId::from_handle(&args.from.dir));
        let to_dir_wcc = self.wcc(ObjectId::from_handle(&args.to.dir));
        let fail = |error| rename::Fail {
            error,
            from_dir_wcc: from_dir_wcc.clone(),
            to_dir_wcc: to_dir_wcc.clone(),
        };

        let from_dir = dir_of(&args.from.dir).map_err(fail)?;
        let to_dir = dir_of(&args.to.dir).map_err(fail)?;
        let (from_name, to_name) = (args.from.name.as_str(), args.to.name.as_str());
        if is_dot(from_name) || is_dot(to_name) {
            return Err(fail(vfs::Error::InvalidArgument));
        }
        // Every name exists, so the target is overwritten: it has to be compatible.
        match (from_dir.child(from_name).kind(), to_dir.child(to_name).kind()) {
            (Kind::Regular, Kind::Directory) => return Err(fail(vfs::Error::IsDir)),
            (Kind::Directory, Kind::Regular) => return Err(fail(vfs::Error::NotDir)),
            _ => {}
        }

        Ok(rename::Success { from_dir_wcc: self.wcc(from_dir), to_dir_wcc: self.wcc(to_dir) })
    }
}

impl read_dir::ReadDir for MockVfs {
    /// Lists as many entries as fit into [`read_dir::Args::count`], honouring cookies.
    async fn read_dir(
        &self,
        args: read_dir::Args,
        _cred: &Credential,
    ) -> Result<read_dir::Success, read_dir::Fail> {
        self.wait(0).await;
        let dir_attr = Some(self.attr_of(&args.dir));
        let fail = |error| read_dir::Fail { error, dir_attr: dir_attr.clone() };

        let dir = dir_of(&args.dir).map_err(fail)?;
        let budget = (args.count as usize).saturating_sub(REPLY_OVERHEAD);
        let (entries, eof) = self.list(dir, args.cookie.raw(), budget, budget, 0).map_err(fail)?;

        let entries = entries
            .into_iter()
            .map(|entry| {
                Ok(read_dir::Entry {
                    file_id: entry.id.raw(),
                    file_name: entry_name(entry.name)?,
                    cookie: Cookie::new(entry.cookie),
                })
            })
            .collect::<Result<Vec<_>, vfs::Error>>()
            .map_err(fail)?;

        Ok(read_dir::Success {
            dir_attr: Some(self.attr(dir)),
            cookie_verifier: CookieVerifier::new([0; 8]),
            entries,
            eof,
        })
    }
}

impl read_dir_plus::ReadDirPlus for MockVfs {
    /// Like `READDIR`, bounded by both [`read_dir_plus::Args::dir_count`] and
    /// [`read_dir_plus::Args::max_count`].
    async fn read_dir_plus(
        &self,
        args: read_dir_plus::Args,
        _cred: &Credential,
    ) -> Result<read_dir_plus::Success, read_dir_plus::Fail> {
        self.wait(0).await;
        let dir_attr = Some(self.attr_of(&args.dir));
        let fail = |error| read_dir_plus::Fail { error, dir_attr: dir_attr.clone() };

        let dir = dir_of(&args.dir).map_err(fail)?;
        let backend = args.dir.backend_id();
        let total_budget = (args.max_count as usize).saturating_sub(REPLY_OVERHEAD);
        let (entries, eof) = self
            .list(dir, args.cookie.raw(), args.dir_count as usize, total_budget, ENTRY_PLUS_EXTRA)
            .map_err(fail)?;

        let entries = entries
            .into_iter()
            .map(|entry| {
                Ok(read_dir_plus::Entry {
                    file_id: entry.id.raw(),
                    file_name: entry_name(entry.name)?,
                    cookie: Cookie::new(entry.cookie),
                    file_attr: Some(self.attr(entry.id)),
                    file_handle: Some(entry.id.handle(backend)),
                })
            })
            .collect::<Result<Vec<_>, vfs::Error>>()
            .map_err(fail)?;

        Ok(read_dir_plus::Success {
            dir_attr: Some(self.attr(dir)),
            cookie_verifier: CookieVerifier::new([0; 8]),
            entries,
            eof,
        })
    }
}
