use std::io::{Read, Write};

use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;

use crate::{
    xdr::{self, ArrayItem, Deserialize, Serialize},
    DeserializeEnum, SerializeEnum,
};

pub mod compound;
pub mod null;

#[allow(dead_code)]
trait Operation {
    fn execute(
        &self,
        context: &mut crate::protocol::rpc::Context,
        cmp: Compound,
    ) -> Result<Response, anyhow::Error>;
}

#[allow(dead_code)]
pub struct Compound {}

#[derive(Debug)]
/// nfs_argop4
pub struct Request {
    //TODO: Походу тут надо добавить что-то для lkhd->flags ??
    pub argop: OpNum,
    pub uin: Argument,
}

impl ArrayItem for Request {}

impl Default for Request {
    fn default() -> Self {
        Self { argop: OpNum::Null, uin: Argument::Null(null::Args::default()) }
    }
}

impl Deserialize for Request {
    fn deserialize<R: Read>(&mut self, src: &mut R) -> std::io::Result<()> {
        self.argop.deserialize(src)?;

        match &self.argop {
            OpNum::Null => {
                let mut arg = null::Args::default();
                arg.deserialize(src)?;
                self.uin = Argument::Null(arg);
            }
            OpNum::Compound => {
                let mut arg = compound::Args::default();
                arg.deserialize(src)?;
                self.uin = Argument::Compound(arg);
            }
            command => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("Not implemented {command:?}"),
                ));
            }
        }

        Ok(())
    }
}

impl Request {
    pub fn deserialize_from_rpc(
        call: xdr::rpc::call_body,
        input: &mut impl Read,
    ) -> Result<Self, anyhow::Error> {
        let argop =
            OpNum::from_u32(call.proc).ok_or(anyhow!("Unknown operation: {:?}", call.proc))?;

        match argop {
            OpNum::Null => {
                let mut args = null::Args::default();
                args.deserialize(input)?;
                Ok(Self { argop, uin: Argument::Null(args) })
            }
            command => Err(anyhow!("Not implemented: {:?}", command)),
        }
    }

    pub fn execute(&self) -> Result<Response, anyhow::Error> {
        match &self.uin {
            Argument::Null(args) => args.execute(),
            Argument::Compound(args) => args.execute(),
            _ => Ok(Response {
                status: Status::ErrInval,
                resop: OpNum::Null,
                uin: Data::Null(Default::default()),
            }),
        }
    }
}

#[derive(Debug)]
/// nfs_resop4
pub struct Response {
    /// Not in RFC NFSv4. Added for simpler architecture
    pub status: Status,
    pub resop: OpNum,
    pub uin: Data,
}

impl xdr::ArrayItem for Response {}

impl Response {
    pub fn serialize_no_resop(&self, dest: &mut impl Write) -> std::io::Result<()> {
        match &self.uin {
            Data::Null(resp) => resp.serialize(dest),
            Data::Compound(resp) => resp.serialize(dest),
        }
    }
}

impl Serialize for Response {
    fn serialize<W: Write>(&self, dest: &mut W) -> std::io::Result<()> {
        self.resop.serialize(dest)?;
        match &self.uin {
            Data::Null(resp) => resp.serialize(dest),
            Data::Compound(resp) => resp.serialize(dest),
        }
    }
}

#[derive(Copy, Clone, Debug, FromPrimitive, ToPrimitive)]
#[repr(u32)]
/// nfs_opnum4
pub enum OpNum {
    Null = 0,
    Compound = 1,
    Access = 3,
    Close = 4,
    Commit = 5,
    Create = 6,
    Delegpurge = 7,
    Delegreturn = 8,
    Getattr = 9,
    Getfh = 10,
    Link = 11,
    Lock = 12,
    Lockt = 13,
    Locku = 14,
    Lookup = 15,
    Lookupp = 16,
    Nverify = 17,
    Open = 18,
    Openattr = 19,
    OpenConfirm = 20,
    OpenDowngrade = 21,
    Putfh = 22,
    Putpubfh = 23,
    Putrootfh = 24,
    Read = 25,
    Readdir = 26,
    Readlink = 27,
    Remove = 28,
    Rename = 29,
    Renew = 30,
    Restorefh = 31,
    Savefh = 32,
    Secinfo = 33,
    Setattr = 34,
    Setclientid = 35,
    SetclientidConfirm = 36,
    Verify = 37,
    Write = 38,
    ReleaseLockowner = 39,
    BackchannelCtl = 40,
    BindConnToSession = 41,
    ExchangeId = 42,
    CreateSession = 43,
    DestroySession = 44,
    FreeStateid = 45,
    GetDirDelegation = 46,
    Getdeviceinfo = 47,
    Getdevicelist = 48,
    Layoutcommit = 49,
    Layoutget = 50,
    Layoutreturn = 51,
    SecinfoNoName = 52,
    Sequence = 53,
    SetSsv = 54,
    TestStateid = 55,
    WantDelegation = 56,
    DestroyClientid = 57,
    ReclaimComplete = 58,
    Allocate = 59,
    Copy = 60,
    CopyNotify = 61,
    Deallocate = 62,
    IoAdvise = 63,
    Layouterror = 64,
    Layoutstats = 65,
    OffloadCancel = 66,
    OffloadStatus = 67,
    ReadPlus = 68,
    Seek = 69,
    WriteSame = 70,
    Clone = 71,
    Getxattr = 72,
    Setxattr = 73,
    Listxattr = 74,
    Removexattr = 75,
    LastOne = 76,
    Illegal = 10044,
}
DeserializeEnum!(OpNum);
SerializeEnum!(OpNum);

#[allow(dead_code)]
#[derive(Debug)]
/// Request arguments
/// nfs_argop4_u
pub enum Argument {
    Null(null::Args),
    Compound(compound::Args),
    Access,
}

#[derive(Debug)]
/// Responses data
/// nfs_resop4_u
pub enum Data {
    Null(null::Resp),
    Compound(compound::Resp),
}

#[derive(Copy, Clone, Debug, Default, FromPrimitive, ToPrimitive, PartialEq, Eq)]
#[repr(u32)]
/// nfsstat4
pub enum Status {
    #[default]
    Ok = 0,
    ErrPerm = 1,
    ErrNoent = 2,
    ErrIo = 5,
    ErrNxio = 6,
    ErrAccess = 13,
    ErrExist = 17,
    ErrXdev = 18,
    ErrNotDir = 20,
    ErrIsDir = 21,
    ErrInval = 22,
    ErrFbig = 27,
    ErrNoSpc = 28,
    ErrRofs = 30,
    ErrMlink = 31,
    ErrNameTooLong = 63,
    ErrNotEmpty = 66,
    ErrDquot = 69,
    ErrStale = 70,
    ErrBadHandle = 10001,
    ErrBadCookie = 10003,
    ErrNotSupp = 10004,
    ErrTooSmall = 10005,
    ErrServerFault = 10006,
    ErrBadType = 10007,
    ErrDelay = 10008,
    ErrSame = 10009,
    ErrDenied = 10010,
    ErrExpired = 10011,
    ErrLocked = 10012,
    ErrGrace = 10013,
    ErrFhexpired = 10014,
    ErrShareDenied = 10015,
    ErrWrongSec = 10016,
    ErrClidInuse = 10017,
    ErrResource = 10018,
    ErrMoved = 10019,
    ErrNoFilehandle = 10020,
    ErrMinorVersMismatch = 10021,
    ErrStaleClientid = 10022,
    ErrStaleStateid = 10023,
    ErrOldStateid = 10024,
    ErrBadStateid = 10025,
    ErrBadSeqid = 10026,
    ErrNotSame = 10027,
    ErrLockRange = 10028,
    ErrSymlink = 10029,
    ErrRestorefh = 10030,
    ErrLeaseMoved = 10031,
    ErrAttrnotsupp = 10032,
    ErrNoGrace = 10033,
    ErrReclaimBad = 10034,
    ErrReclaimConflict = 10035,
    ErrBadxdr = 10036,
    ErrLocksHeld = 10037,
    ErrOpenmode = 10038,
    ErrBadowner = 10039,
    ErrBadchar = 10040,
    ErrBadname = 10041,
    ErrBadRange = 10042,
    ErrLockNotsupp = 10043,
    ErrOpIllegal = 10044,
    ErrDeadlock = 10045,
    ErrFileOpen = 10046,
    ErrAdminRevoked = 10047,
    ErrCbPathDown = 10048,
    ErrBadiomode = 10049,
    ErrBadlayout = 10050,
    ErrBadSessionDigest = 10051,
    ErrBadsession = 10052,
    ErrBadslot = 10053,
    ErrCompleteAlready = 10054,
    ErrConnNotBoundToSession = 10055,
    ErrDelegAlreadyWanted = 10056,
    ErrBackChanBusy = 10057,
    ErrLayouttrylater = 10058,
    ErrLayoutunavailable = 10059,
    ErrNomatchingLayout = 10060,
    ErrRecallconflict = 10061,
    ErrUnknownLayouttype = 10062,
    ErrSeqMisordered = 10063,
    ErrSequencePos = 10064,
    ErrReqTooBig = 10065,
    ErrRepTooBig = 10066,
    ErrRepTooBigToCache = 10067,
    ErrRetryUncachedRep = 10068,
    ErrUnsafeCompound = 10069,
    ErrTooManyOps = 10070,
    ErrOpNotInSession = 10071,
    ErrHashAlgUnsupp = 10072,
    ErrClientidBusy = 10074,
    ErrPnfsIoHole = 10075,
    ErrSeqFalseRetry = 10076,
    ErrBadHighSlot = 10077,
    ErrDeadsession = 10078,
    ErrEncrAlgUnsupp = 10079,
    ErrPnfsNoLayout = 10080,
    ErrNotOnlyOp = 10081,
    ErrWrongCred = 10082,
    ErrWrongType = 10083,
    ErrDirdelegUnavail = 10084,
    ErrRejectDeleg = 10085,
    ErrReturnconflict = 10086,
    ErrDelegRevoked = 10087,
    ErrPartnerNotsupp = 10088,
    ErrPartnerNoAuth = 10089,
    ErrUnionNotsupp = 10090,
    ErrOffloadDenied = 10091,
    ErrWrongLfs = 10092,
    ErrBadlabel = 10093,
    ErrOffloadNoReqs = 10094,
    ErrNoxattr = 10095,
    ErrXattr2big = 10096,
    ErrReplay = 11001,
}
SerializeEnum!(Status);
