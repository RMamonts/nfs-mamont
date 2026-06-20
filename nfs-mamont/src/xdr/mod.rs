pub use nfs_mamont_derive::XDRSize;
use std::path::PathBuf;

pub trait XDRSize {
    const ALIGNMENT: usize = 4;
    const INTEGER: usize = 4;
    const HYPER_INTEGER: usize = 8;

    fn xdr_size(&self) -> usize;
}

impl XDRSize for u32 {
    fn xdr_size(&self) -> usize {
        Self::INTEGER
    }
}

impl XDRSize for i32 {
    fn xdr_size(&self) -> usize {
        Self::INTEGER
    }
}

impl XDRSize for u64 {
    fn xdr_size(&self) -> usize {
        Self::HYPER_INTEGER
    }
}

impl XDRSize for usize {
    fn xdr_size(&self) -> usize {
        //sure?
        Self::INTEGER
    }
}

impl<const N: usize> XDRSize for [u8; N] {
    fn xdr_size(&self) -> usize {
        (N + (Self::ALIGNMENT - 1)) & !(Self::ALIGNMENT - 1)
    }
}

impl XDRSize for Vec<u8> {
    fn xdr_size(&self) -> usize {
        Self::INTEGER + ((self.len() + (Self::ALIGNMENT - 1)) & !(Self::ALIGNMENT - 1))
    }
}

impl<T: XDRSize> XDRSize for Vec<T> {
    fn xdr_size(&self) -> usize {
        Self::INTEGER + self.iter().map(|item| item.xdr_size()).sum::<usize>()
    }
}

impl XDRSize for bool {
    fn xdr_size(&self) -> usize {
        Self::INTEGER
    }
}

impl<T: XDRSize> XDRSize for Option<T> {
    fn xdr_size(&self) -> usize {
        match self {
            Some(x) => true.xdr_size() + x.xdr_size(),
            None => false.xdr_size(),
        }
    }
}

impl XDRSize for String {
    fn xdr_size(&self) -> usize {
        Self::INTEGER + ((self.len() + (Self::ALIGNMENT - 1)) & !(Self::ALIGNMENT - 1))
    }
}

impl XDRSize for PathBuf {
    fn xdr_size(&self) -> usize {
        let path_str = self.to_string_lossy();
        Self::INTEGER + ((path_str.len() + (Self::ALIGNMENT - 1)) & !(Self::ALIGNMENT - 1))
    }
}

//TODO: more tests!!!
#[cfg(test)]
mod tests {
    use super::XDRSize;

    #[derive(XDRSize)]
    struct Pair {
        a: u32,
        b: u64,
    }

    #[derive(XDRSize)]
    enum Choice {
        A,
        B(u32),
        C { x: u32, y: u64 },
    }

    #[test]
    fn derive_struct_sums_field_sizes() {
        let pair = Pair { a: 1, b: 2 };
        assert_eq!(pair.xdr_size(), 4 + 8);
    }

    #[test]
    fn derive_enum_includes_discriminant() {
        assert_eq!(Choice::A.xdr_size(), 4);
        assert_eq!(Choice::B(0).xdr_size(), 4 + 4);
        assert_eq!(Choice::C { x: 0, y: 0 }.xdr_size(), 4 + 4 + 8);
    }
}
