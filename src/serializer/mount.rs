use crate::vfs::{FileHandle, FileName, FsPath};

#[allow(dead_code)]
pub enum MountStat {
    OK = 0,              /* no error */
    Perm = 1,            /* Not owner */
    NoEnt = 2,           /* No such file or directory */
    IO = 5,              /* I/O error */
    Access = 13,         /* Permission denied */
    NotDir = 20,         /* Not a directory */
    Invalid = 22,        /* Invalid argument */
    NameTooLong = 63,    /* Filename too long */
    NotSupp = 10004,     /* Operation not supported */
    ServerFault = 10006, /* A failure on the server */
}

#[allow(dead_code)]
pub struct MountResOk {
    fhandle: FileHandle,
    auth_flavors: Vec<u32>,
}

#[allow(dead_code)]
pub struct MountBody {
    ml_hostname: FileName,
    ml_directory: FsPath,
    ml_next: Option<Box<MountBody>>,
}

#[allow(dead_code)]
struct GroupNode {
    gr_name: FileName,
    groups: Option<Box<GroupNode>>,
}

#[allow(dead_code)]
struct ExportNode {
    ex_dir: FsPath,
    groups: Option<GroupNode>,
    exports: Option<Box<ExportNode>>,
}
