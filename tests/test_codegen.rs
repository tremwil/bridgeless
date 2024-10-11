//#![no_std]

use bridgeless::*;

// struct LtStruct<'a> {
//     _data: &'a [u8],
// }

// #[vtable]
// pub trait Test {
//     fn returns_ref_implicit(&self) -> &u32;
//     fn returns_lt_bound_struct(&self) -> LtStruct<'_>;
//     fn returns_ref_complex<'a, 'b>(&'a self, other: &'b u32) -> &'a u32;
// }

// #[vtable]
// pub trait Base {
//     fn a(&self, arg1: u32) -> bool {
//         false
//     }
//     unsafe fn b(&self, arg1: *const u8, arg2: u32) -> bool;
// }

// #[vtable]
// pub(crate) trait Derived: Base {
//     fn c(&mut self) -> usize;
// }

// #[derive(Default, Debug)]
// struct DerivedImpl {
//     // VPtr<dyn T, Self: T> supports `Default`, so your compile-time generated vtable
//     // can be automatically provided to the object
//     vftable: VmtPtr<dyn Derived, Self>,
// }

// impl Base for DerivedImpl {
//     extern "C" fn a(&self, arg1: u32) -> bool {
//         arg1 == 42
//     }

//     unsafe extern "C" fn b(&self, _arg1: *const u8, _arg2: u32) -> bool {
//         false
//     }
// }

// impl Derived for DerivedImpl {
//     extern "C" fn c(&mut self) -> usize {
//         1234
//     }
// }

// #[test]
// fn default_derived_impls_correct() {
//     let mut d = DerivedImpl::default();

//     let vmt_copy = d.vftable;

//     assert_eq!(d.a(42), (d.vftable.a)(&d, 42));

//     unsafe {
//         let s = b"abc";
//         let ptr = s.as_ptr();
//         assert_eq!(d.b(ptr, 420), (d.vftable.b)(&d, ptr, 420));
//     }

//     assert_eq!(d.c(), (d.vftable.c)(&mut d))
// }

#[repr(C)]
pub struct BaseData {
    pub a: u32,
}

pub trait Base: 'static {
    extern "C" fn virtual_fn(&self) -> u32;
}

#[repr(C)]
pub struct BaseVtable<T: 'static> {
    virtual_fn: extern "C" fn(this: &T) -> &u32,
}

impl<T: 'static> Clone for BaseVtable<T> {
    fn clone(&self) -> Self {
        Self {
            virtual_fn: self.virtual_fn,
        }
    }
}

impl<T: 'static> Copy for BaseVtable<T> {}

unsafe impl Class for dyn Base {
    type Vmt<T: 'static> = BaseVtable<T>;
    type Data = BaseData;
}

unsafe impl<T: Base + Class + ?Sized> internal::SubclassOf<dyn Base>
    for internal::SubclassOfWrapper<T>
{
}

fn test(x: CRefMut<'_, dyn Base>, y: CRefMut<'_, dyn Base>) {}

#[repr(C)]
pub struct Derived {
    _base: BaseData,
    b: usize,
}

#[repr(C)]
pub struct Derived2 {
    _base: BaseData,
    b: usize,
}
