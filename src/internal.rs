use core::{marker::PhantomData, ops::Add};

use crate::Class;

/// Trait implemented by types that are wrappers around a class layout,
/// such as `Cls`, `DynCls`, and `Impl`.
pub trait ClassWrapper {
    /// The class this wrapper is representing the layout of.
    type ClsType: Class;
}

/// Converts a thin type erased pointer to a (potentially fat) typed pointer.
pub trait FromThinPtr {
    unsafe fn from_thin_ptr(ptr: *const u8) -> *const Self;
    unsafe fn from_thin_ptr_mut(ptr: *mut u8) -> *mut Self;
}

// This is a way to get around
// https://rust-lang.github.io/rfcs/2451-re-rebalancing-coherence.html#concrete-orphan-rules
//
// Generated code for Base can't blanket impl SubclassOf<dyn Base> for T, but they can derive
// internal::SubclassOf<dyn Base> for internal::SubclassOfWrapper<T>. Then the blanket impl
// implements SubclassOf<dyn Base> for T.

pub unsafe trait SubclassOf<B: Class + ?Sized>: 'static {}

pub struct SubclassOfWrapper<T: ?Sized>(T);
unsafe impl<B: Class + ?Sized, D: Class + ?Sized> crate::SubclassOf<B> for D where
    SubclassOfWrapper<D>: SubclassOf<B>
{
}

pub struct FallbackVmtGen<O, F>(PhantomData<(O, F)>);

/// Trait around a type with a usize associated const.
///
/// This is required to be able to do math on const generics without the generic_const_exprs
/// experimental feature.
pub trait HasConst<T> {
    const VALUE: T;
}

pub struct ConstUsizeValue<const N: usize>;
impl<const N: usize> HasConst<usize> for ConstUsizeValue<N> {
    const VALUE: usize = N;
}

/// Type implementing [`ConstUsize`] which adds a constant to an existing [`ConstUsize`].
pub struct AddConst<T: HasConst<usize>, const N: usize>(PhantomData<T>);
impl<T: HasConst<usize>, const N: usize> HasConst<usize> for AddConst<T, N> {
    const VALUE: usize = N + T::VALUE;
}

pub struct AddConsts<U: HasConst<usize>, V: HasConst<usize>>(PhantomData<(U, V)>);
impl<U: HasConst<usize>, V: HasConst<usize>> HasConst<usize> for AddConsts<U, V> {
    const VALUE: usize = U::VALUE + V::VALUE;
}

/// Trait implemented by certain type configurations of [`Class::VmtSource`] to provide a
/// fallback "specialization" of `C`'s function table that allows for C++ like method inhertiance.
///
/// # SAFETY
/// **This trait should not be implemented manually**. Invariants that
/// guarantee soundness are not stable and may change subtly between versions.
pub unsafe trait VmtPartGen<C: Class> {
    type ForOffset<Ofs: HasConst<usize>>: HasConst<C::VmtPart>;
}

/// Given a list of the size of the virtual function tables of all base classes,
/// checks if the order is compatible with the C++ ABI. If not, will panic to prevent
/// compilation and let the user know.
pub const fn assert_base_ordering(size_of_vmts: &[usize]) {
    match size_of_vmts.first() {
        Some(0) => (),
        _ => return,
    };

    let mut i = 1;
    while i < size_of_vmts.len() {
        if size_of_vmts[i] > 0 {
            panic!("Invalid base class ordering: When any base has a vtable, the first base must also have one")
        }

        i += 1;
    }
}
