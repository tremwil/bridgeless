use crate::Class;

// This is a way to get around
// https://rust-lang.github.io/rfcs/2451-re-rebalancing-coherence.html#concrete-orphan-rules
//
// Generated code for Base can't blanket impl SubclassOf<dyn Base> for T, but they can derive
// internal::SubclassOf<dyn Base> for internal::SubclassOfWrapper<T>. Then the blanket impl implements
// SubclassOf<dyn Base> for T.

pub unsafe trait SubclassOf<B: Class + ?Sized>: 'static {}

pub struct SubclassOfWrapper<T: ?Sized>(T);
unsafe impl<B: Class + ?Sized, D: ?Sized> crate::SubclassOf<B> for D where
    SubclassOfWrapper<D>: SubclassOf<B>
{
}

/// Compile-time function indicating whether or not a given [`Class`]
/// has a virtual method table.
pub const fn has_vmt<C: Class + ?Sized>() -> bool {
    core::mem::size_of::<C::Vmt<()>>() > 0
}
