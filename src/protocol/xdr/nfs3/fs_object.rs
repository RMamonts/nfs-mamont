use crate::xdr::nfs3::{diropargs3, nfs_fh3, nfstime3, sattr3};
use crate::xdr::Deserialize;
use crate::xdr::Serialize;
use crate::{DeserializeStruct, SerializeStruct};
use std::io::Read;
use std::io::Write;
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct ACCESS3args {
    pub object: nfs_fh3,
    pub access: u32,
}
SerializeStruct!(ACCESS3args, object, access);
DeserializeStruct!(ACCESS3args, object, access);
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct GETATTR3args {
    pub object: nfs_fh3,
}
SerializeStruct!(GETATTR3args, object);
DeserializeStruct!(GETATTR3args, object);

pub type sattrguard3 = Option<nfstime3>;

/// Arguments for `SETATTR` operations
#[allow(non_camel_case_types)]
#[derive(Clone, Debug, Default)]
pub struct SETATTR3args {
    /// File handle for target file
    pub object: nfs_fh3,
    /// New attributes to set
    pub new_attribute: sattr3,
    /// Guard condition for atomic change
    pub guard: Option<nfstime3>,
}
DeserializeStruct!(SETATTR3args, object, new_attribute, guard);
SerializeStruct!(SETATTR3args, object, new_attribute, guard);

#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct RENAME3args {
    pub from: diropargs3,
    pub to: diropargs3,
}
SerializeStruct!(RENAME3args, from, to);
DeserializeStruct!(RENAME3args, from, to);

#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct READLINK3args {
    pub symlink: nfs_fh3,
}
SerializeStruct!(READLINK3args, symlink);
DeserializeStruct!(READLINK3args, symlink);
