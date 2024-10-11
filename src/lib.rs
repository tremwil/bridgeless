#![no_std]

use core::ops::{Deref, DerefMut};

//pub use bridgeless_proc_macros::vtable;

pub mod internal;

/// Metadata trait specifying the layout of a (single-inheritance) C++ class.
///
/// # SAFETY
/// This trait is unsafe as it's associated types must abide by the following
/// contracts to prevent undefined behavior:
///
/// 1. `Vmt<T>` and `Data` must be `#[repr(C)]`.
/// 2. Implementor must be `dyn X` for some trait `X`.
/// 3. For all `Y: ?Sized + Class + X`:
///     - The first field of `Y::Data` must be `X::Data`;
///     - The first field of `Y::Vmt<T>` must be `X::Vmt<T>`.
pub unsafe trait Class: 'static + SubclassOf<Self> {
    /// The layout of a virtual method table where the self type is T.
    type Vmt<T: 'static>: 'static + Copy;

    /// The layout of the class's fields, excluding the virtual method table.
    type Data: 'static;
}

/// Trait which provides a static reference to the virtual function table of
/// a concrete type T for the [`Class`] that implements it.
pub trait VmtReference<T: 'static>: 'static + Class {
    const VTABLE: &'static Self::Vmt<T>;
}

/// Custom marker trait signifying that a given [`Class`] is an (inclusive) subclass of `B`.
///
/// # SAFETY
/// (TODO)
pub unsafe trait SubclassOf<B: Class + ?Sized>: 'static {}

/// Layout of a concrete object instance.
#[repr(C)]
pub struct Obj<C: Class + ?Sized> {
    vtable: &'static C::Vmt<C::Data>,
    data: C::Data,
}

pub struct CRef<'a, C: Class + ?Sized>(&'a Obj<C>);
pub struct CRefMut<'a, C: Class + ?Sized>(&'a mut Obj<C>);

impl<C: Class + ?Sized> Obj<C> {
    pub fn cast<As: Class + ?Sized>(&self) -> CRef<'_, As>
    where
        C: SubclassOf<As>,
    {
        // We have a VMT, but not the base. Need to bump the pointer!
        if internal::has_vmt::<C>() ^ internal::has_vmt::<As>() {
            todo!()
        }
        else {
            unsafe { core::mem::transmute(self) }
        }
    }

    pub fn cast_mut<As: Class + ?Sized>(&mut self) -> CRefMut<'_, As>
    where
        C: SubclassOf<As>,
    {
        // We have a VMT, but not the base. Need to bump the pointer!
        if internal::has_vmt::<C>() ^ internal::has_vmt::<As>() {
            todo!()
        }
        else {
            unsafe { core::mem::transmute(self) }
        }
    }
}

// #[cxx_class]
// #[derive(Clone)]
// struct MyStruct {
// }

// impl MyStruct {

// }

/// Gets a static reference to the vtable of a the given concrete type, computed at
/// compile time.
pub const fn vmt_instance<V: VmtReference<T> + ?Sized, T>() -> &'static V::Vmt<T> {
    V::VTABLE
}
