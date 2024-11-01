use crate::Class;

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
