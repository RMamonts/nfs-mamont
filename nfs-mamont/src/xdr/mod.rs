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
        (N + Self::ALIGNMENT) & !Self::ALIGNMENT
    }
}

impl XDRSize for Vec<u8> {
    fn xdr_size(&self) -> usize {
        Self::INTEGER + ((self.len() + Self::ALIGNMENT) & !Self::ALIGNMENT)
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
