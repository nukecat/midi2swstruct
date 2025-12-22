#![feature(generic_const_exprs)]

mod private {
    pub trait Sealed {}
    impl Sealed for super::Boolean<true> {}
    impl Sealed for super::Boolean<false> {}
}

pub trait IsTrue: private::Sealed {}
pub trait IsFalse: private::Sealed {}

pub struct Boolean<const B: bool> {}

impl IsTrue for Boolean<true> {}
impl IsFalse for Boolean<false> {}

/// Example usage.
struct foo<const N: usize>
where Boolean<{N != 5}>: IsTrue {}
