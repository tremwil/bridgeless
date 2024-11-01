// #![no_std]

use std::marker::PhantomData;

use bridgeless::*;
use internal::FromThinPtr;

mod test {

    pub use bridgeless::foreign::*;

    pub struct A;
    pub trait AFwd: TraitOf<A> {}
    impl MyDynTrait for A {
        type T = dyn AFwd;
    }
    impl<T: MyDynTrait> InternalTraitOf<A> for Wrapper<T> where <T as MyDynTrait>::T: TraitOf<A> {}

    pub struct B;
    pub trait BFwd: TraitOf<B> {}
    impl MyDynTrait for B {
        type T = dyn BFwd;
    }
    impl<T: MyDynTrait> InternalTraitOf<B> for Wrapper<T> where <T as MyDynTrait>::T: TraitOf<B> {}

    pub struct C;
    pub trait CFwd: TraitOf<A> + TraitOf<B> + TraitOf<C> {}
    impl MyDynTrait for C {
        type T = dyn CFwd;
    }
    impl<T: MyDynTrait> InternalTraitOf<C> for Wrapper<T> where <T as MyDynTrait>::T: TraitOf<C> {}

    fn test(x: impl TraitOf<B>) {}

    fn test2() {
        test(C)
    }
}

//impl<T, Tail: Contains<T>> Contains<T> for TNode<T, Tail> {}

#[allow(non_camel_case_types)]
trait A_Impl: 'static {
    fn virt_a(&mut self) -> usize {
        unimplemented!()
    }
}

trait A: A_Impl {}

#[repr(C)]
#[derive(Clone, Copy)]
struct AVmt(for<'a> unsafe extern "C" fn(&'a mut u8) -> usize);

#[repr(C)]
struct AData {
    a_field: usize,
}

#[repr(C)]
struct ALayout<V: 'static>(&'static V, AData);
impl<V: 'static> AsRef<AData> for ALayout<V> {
    fn as_ref(&self) -> &AData {
        &self.1
    }
}
impl<V: 'static> AsMut<AData> for ALayout<V> {
    fn as_mut(&mut self) -> &mut AData {
        &mut self.1
    }
}

unsafe impl Class for dyn A {
    type Data = AData;
    type Vmt = AVmt;
    type Layout<V: 'static> = ALayout<V>;
    type VmtSource<T: 'static + FromThinPtr + ?Sized, const OFFSET: usize> =
        AVmtSource<T, OFFSET, Impl<dyn A>>;

    fn base_offset<C: Class + ?Sized>() -> Option<usize> {
        unimplemented!()
    }
}

unsafe impl<C: Class + A + ?Sized> internal::SubclassOf<dyn A> for internal::SubclassOfWrapper<C> {}

pub struct AVmtSource<T: 'static + FromThinPtr + ?Sized, const OFFSET: usize, I: 'static + ?Sized>(
    PhantomData<fn() -> (&'static I, &'static T)>,
);

struct AThunkGenerator<T: A_Impl + FromThinPtr + ?Sized, const OFFSET: usize>(
    PhantomData<fn() -> &'static T>,
);
impl<T: A_Impl + FromThinPtr + ?Sized, const OFFSET: usize> AThunkGenerator<T, OFFSET> {
    unsafe extern "C" fn virt_a(thisptr: &mut u8) -> usize {
        let instance_ptr = (thisptr as *mut u8).sub(OFFSET);
        (&mut *T::from_thin_ptr_mut(instance_ptr)).virt_a()
    }
}

unsafe impl<T: FromThinPtr + ?Sized, const OFFSET: usize, I> BaseVtableFor<dyn A>
    for AVmtSource<T, OFFSET, I>
where
    I: A_Impl + FromThinPtr + ?Sized,
{
    const VTABLE: <dyn A as Class>::Vmt = AVmt(AThunkGenerator::<I, OFFSET>::virt_a);
    const VTABLE_REF: &'static <dyn A as Class>::Vmt = &AVmt(AThunkGenerator::<I, OFFSET>::virt_a);
}

impl<T: A_Impl + FromThinPtr + ?Sized, const OFFSET: usize> AVmtSource<T, OFFSET, Impl<dyn A>> {
    pub const VTABLE: <dyn A as Class>::Vmt = AVmt(AThunkGenerator::<T, OFFSET>::virt_a);
    pub const VTABLE_REF: &'static <dyn A as Class>::Vmt =
        &AVmt(AThunkGenerator::<T, OFFSET>::virt_a);
}

impl<C: A + Class + ?Sized> A_Impl for DynCls<C> {
    fn virt_a(&mut self) -> usize {
        let base_offset = C::base_offset::<dyn A>().expect("Unreachable code ran");
        unsafe {
            let thin_ptr = (self as *mut _ as *mut u8).add(base_offset);
            let func = (*(thin_ptr as *mut AVmt)).0;
            (func)(&mut *thin_ptr)
        }
    }
}

impl A_Impl for Impl<dyn A> {
    fn virt_a(&mut self) -> usize {
        0
    }
}

struct Derived {}
impl FromThinPtr for Derived {
    unsafe fn from_thin_ptr(ptr: *const u8) -> *const Self {
        todo!()
    }
    unsafe fn from_thin_ptr_mut(ptr: *mut u8) -> *mut Self {
        todo!()
    }
}

const fn test() -> &'static AVmt {
    <dyn A as Class>::VmtSource::<Derived, 0>::VTABLE_REF
}
