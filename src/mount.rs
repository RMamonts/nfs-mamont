#![allow(dead_code)]
#![allow(non_camel_case_types, clippy::upper_case_acronyms)]

const MOUNT_PROGRAM: u32 = 100005;
const MOUNT_VERSION: u32 = 3;

const MNTPATHLEN: u32 = 1024;
const MNTNAMLEN: u32 = 255;
const FHSIZE3: u32 = 64;

type fhandle3 = Vec<u8>;
type dirpath = Vec<u8>;
type name = Vec<u8>;

#[derive(Debug)]
#[repr(u32)]
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

#[derive(Debug)]
#[repr(u32)]
enum MountProgram {
    MOUNTPROC3_NULL = 0,
    MOUNTPROC3_MNT = 1,
    MOUNTPROC3_DUMP = 2,
    MOUNTPROC3_UMNT = 3,
    MOUNTPROC3_UMNTALL = 4,
    MOUNTPROC3_EXPORT = 5,
}

#[derive(Debug)]
struct mountres3_ok {
    fhandle: fhandle3,
    auth_flavors: Vec<i32>,
}

type mountlist = Option<Box<mountbody>>;

#[derive(Debug)]
struct mountbody {
    ml_hostname: name,
    ml_directory: dirpath,
    ml_next: mountlist,
}

type groups = Option<Box<groupnode>>;

#[derive(Debug)]
struct groupnode {
    gr_name: name,
    gr_next: groups,
}

type exports = Option<Box<exportnode>>;

#[derive(Debug)]
struct exportnode {
    ex_dir: dirpath,
    ex_groups: groups,
    ex_next: exports,
}
