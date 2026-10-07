pub(crate) trait ToU32 {
    fn to_u32(&self) -> u32;
}

impl ToU32 for usize {
    fn to_u32(&self) -> u32 {
        #[expect(
            clippy::expect_used,
            reason = "Indices in daedalus language have to fit in 32 bits"
        )]
        u32::try_from(*self).expect("Indices in daedalus language have to fit in 32 bits")
    }
}

pub(crate) trait ToUsize {
    fn to_usize(&self) -> usize;
}

impl ToUsize for u32 {
    fn to_usize(&self) -> usize {
        const { assert!(align_of::<usize>() >= 4, "usize has to be at least 32 bits") };
        usize::try_from(*self).unwrap()
    }
}
