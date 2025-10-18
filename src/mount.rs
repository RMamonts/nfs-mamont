#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

use crate::parser::to_parse::{StringWithMaxLen, VecWithMaxLen};

#[allow(dead_code)]
const MOUNT_PROGRAM: u32 = 100005;
#[allow(dead_code)]
const MOUNT_VERSION: u32 = 3;
#[allow(dead_code)]
const MNTPATHLEN: usize = 1024;
#[allow(dead_code)]
const MNTNAMLEN: usize = 255;
#[allow(dead_code)]
const FHSIZE3: usize = 64;

type fhandle3 = VecWithMaxLen<FHSIZE3>;
type dirpath = StringWithMaxLen<MNTPATHLEN>;
type name = StringWithMaxLen<MNTNAMLEN>;

#[allow(dead_code)]
enum mountstat3 {
    MNT3_OK = 0,
    MNT3ERR_PERM = 1,
    MNT3ERR_NOENT = 2,
    MNT3ERR_IO = 5,
    MNT3ERR_ACCES = 13,
    MNT3ERR_NOTDIR = 20,
    MNT3ERR_INVAL = 22,
    MNT3ERR_NAMETOOLONG = 63,
    MNT3ERR_NOTSUPP = 10004,
    MNT3ERR_SERVERFAULT = 10006,
}

#[allow(dead_code)]
#[repr(u32)]
enum MountProgram {
    MOUNTPROC3_NULL = 0,
    MOUNTPROC3_MNT(dirpath) = 1,
    MOUNTPROC3_DUMP = 2,
    MOUNTPROC3_UMNT(dirpath) = 3,
    MOUNTPROC3_UMNTALL = 4,
    MOUNTPROC3_EXPORT = 5,
}

#[allow(dead_code)]
struct mountres3_ok {
    fhandle: fhandle3,
    auth_flavors: Vec<i32>,
}

type mountlist = Option<Box<mountbody>>;

#[allow(dead_code)]
struct mountbody {
    ml_hostname: name,
    ml_directory: dirpath,
    ml_next: mountlist,
}

type groups = Option<Box<groupnode>>;

#[allow(dead_code)]
struct groupnode {
    gr_name: name,
    gr_next: groups,
}

type exports = Option<Box<exportnode>>;

#[allow(dead_code)]
struct exportnode {
    ex_dir: dirpath,
    ex_groups: groups,
    ex_next: exports,
}
