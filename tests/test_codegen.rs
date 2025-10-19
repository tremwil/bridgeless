// #![no_std]

use std::marker::PhantomData;

use bridgeless::*;
use internal::{AddConst, ConstUsizeValue, FallbackVmtGen, FromThinPtr, HasConst, VmtPartGen};

struct MyBase<T>(T);

// #[class]
// trait MyBase<T> {
//     #[offset(2)]
//     fn fooM<'a>(&'a self) -> usize {
//         self
//     }

//     fn bar(&self);

//     #[offset(10)]
//     fn girth<'a>(&'a self) -> usize {}
// }

#[repr(C)]
pub struct A {
    a_field: usize,
}

#[allow(non_snake_case)]
pub mod A_Meta {
    use bridgeless::internal::{HasConst, VmtPartGen};

    use super::A;
    type Data = A;

    // This is needed to get SubclassOf<A> to work
    pub trait InheritTrait {}

    // This trait alias will be used by derived classes to correctly bound the function that
    // generates instances
    pub trait HasVmtParts: VmtPartGen<A> {}
    impl<T> HasVmtParts for T where T: VmtPartGen<A> {}
}

#[allow(non_camel_case_types)]
trait A_Impl: 'static + bridgeless::internal::ClassWrapper {
    fn virt_a(&mut self) -> usize {
        let base_offset = <Self as bridgeless::internal::ClassWrapper>::ClsType::base_offset::<A>()
            .expect("Unreachable code ran");
        unsafe {
            let thin_ptr = (self as *mut _ as *mut u8).add(base_offset);
            let func = (*(thin_ptr as *mut AVmt)).virt_a.unwrap_unchecked();
            (func)(&mut *thin_ptr)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AVmt {
    pub virt_a: Option<for<'a> unsafe extern "C" fn(&'a mut u8) -> usize>,
}

impl AVmt {
    pub const fn default() -> Self {
        AVmt { virt_a: None }
    }

    pub const fn assert_implemented(&self) {
        self.virt_a.expect("Can't generate vtable for A: missing impl for virt_a");
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ACombinedVmt(pub AVmt);

#[repr(C)]
pub struct ALayout<VPtr: 'static + Copy>(pub VPtr, pub A);
impl<VPtr: 'static + Copy> ALayout<VPtr> {
    pub fn replace_vptr<V: 'static + Copy>(self, new_vptr: V) -> ALayout<V> {
        ALayout(new_vptr, self.1)
    }
}

impl<VPtr: 'static + Copy> ClassLayout<VPtr> for ALayout<VPtr> {
    type Data = A;

    fn data(&self) -> &Self::Data {
        &self.1
    }
    fn data_mut(&mut self) -> &mut Self::Data {
        &mut self.1
    }
    fn vtable(&self) -> VPtr {
        self.0
    }
    unsafe fn vtable_mut(&mut self) -> &mut VPtr {
        &mut self.0
    }
}

unsafe impl Class for A {
    type _InheritTrait = dyn A_Meta::InheritTrait;

    type VmtPart = AVmt;
    type Vmt = ACombinedVmt;
    type VmtPtr = &'static ACombinedVmt;
    type Layout<VPtr: 'static + Copy> = ALayout<VPtr>;

    #[inline(always)]
    fn base_offset<C: Class + ?Sized>() -> Option<usize> {
        if std::any::TypeId::of::<C>() == std::any::TypeId::of::<A>() {
            Some(0)
        } else {
            None
        }
    }
}

unsafe impl<C: Class> internal::SubclassOf<A> for internal::SubclassOfWrapper<C> where
    <C as Class>::_InheritTrait: A_Meta::InheritTrait
{
}

pub struct FallbackGenA<Ofs: HasConst<usize>, O: VmtPartGen<A>, F: VmtPartGen<A>>(
    PhantomData<(Ofs, O, F)>,
);
impl<Ofs: HasConst<usize>, O: VmtPartGen<A>, F: VmtPartGen<A>> HasConst<AVmt>
    for FallbackGenA<Ofs, O, F>
{
    const VALUE: AVmt = AVmt {
        virt_a: match <O::ForOffset<Ofs> as HasConst<AVmt>>::VALUE.virt_a {
            Some(fun) => Some(fun),
            None => <F::ForOffset<Ofs> as HasConst<AVmt>>::VALUE.virt_a,
        },
    };
}

unsafe impl<O: VmtPartGen<A>, F: VmtPartGen<A>> VmtPartGen<A> for internal::FallbackVmtGen<O, F> {
    type ForOffset<Ofs: HasConst<usize>> = FallbackGenA<Ofs, O, F>;
}

impl<C: SubclassOf<A>> A_Impl for DynCls<C> {}
impl<C: SubclassOf<A>> A_Impl for Cls<C> {}

impl A {
    pub fn new(data: A) -> Cls<A> {
        const A_VMT: &ACombinedVmt = &A::make_vmt::<ConstUsizeValue<0>, A>();
        unsafe { Cls::from_layout(ALayout(A_VMT, data)) }
    }

    pub const fn make_vmt<Ofs: HasConst<usize>, G: A_Meta::HasVmtParts>() -> ACombinedVmt {
        let vmt =
            <<FallbackVmtGen<G, A> as VmtPartGen<A>>::ForOffset<Ofs> as HasConst<AVmt>>::VALUE;
        vmt.assert_implemented();
        ACombinedVmt(vmt)
    }
}

impl A_Impl for Impl<A> {
    fn virt_a(&mut self) -> usize {
        42
    }
}
pub struct AThunkGen<Ofs: HasConst<usize>>(PhantomData<Ofs>);
impl<Ofs: HasConst<usize>> HasConst<AVmt> for AThunkGen<Ofs> {
    const VALUE: AVmt = {
        unsafe extern "C" fn virt_a<Ofs: HasConst<usize>>(thisptr: &mut u8) -> usize {
            let derived_ptr = (thisptr as *mut u8).sub(Ofs::VALUE);
            let derived: &mut Impl<A> = &mut *FromThinPtr::from_thin_ptr_mut(derived_ptr);
            A_Impl::virt_a(derived)
        }
        AVmt {
            virt_a: Some(virt_a::<Ofs>),
            ..AVmt::default()
        }
    };
}
unsafe impl VmtPartGen<A> for A {
    type ForOffset<Ofs: HasConst<usize>> = AThunkGen<Ofs>;
}

#[repr(C)]
pub struct B {
    b_field: usize,
}

#[allow(non_snake_case)]
pub mod B_Meta {
    use bridgeless::internal::{HasConst, VmtPartGen};

    use super::B;
    use crate::A_Meta;
    type Data = B;

    pub trait InheritTrait: A_Meta::InheritTrait {}

    pub trait HasVmtParts: VmtPartGen<B> + A_Meta::HasVmtParts {}
    impl<T> HasVmtParts for T where T: VmtPartGen<B> + A_Meta::HasVmtParts {}
}

#[allow(non_camel_case_types)]
trait B_Impl: bridgeless::internal::ClassWrapper
where
    for<'a> Self: 'a,
{
    fn virt_b(&mut self) -> usize {
        unimplemented!()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BVmt {
    pub virt_b: Option<for<'a> unsafe extern "C" fn(&'a mut u8) -> usize>,
}

impl BVmt {
    pub const fn default() -> Self {
        BVmt { virt_b: None }
    }

    pub const fn assert_implemented(&self) {
        self.virt_b.expect("Can't generate vtable for B: missing impl for virt_b");
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BCombinedVmt(pub <A as Class>::Vmt, pub BVmt);

#[repr(C)]
pub struct BLayout<VPtr: 'static + Copy>(pub <A as Class>::Layout<VPtr>, pub B);
impl<VPtr: 'static + Copy> BLayout<VPtr> {
    pub fn replace_vptr<V: 'static + Copy>(self, new_vptr: V) -> BLayout<V> {
        //let a = self.0.replace_vptr::<V>(new_vptr);
        BLayout(self.0.replace_vptr(new_vptr), self.1)
    }
}

impl<VPtr: 'static + Copy> ClassLayout<VPtr> for BLayout<VPtr> {
    type Data = B;

    fn data(&self) -> &Self::Data {
        &self.1
    }
    fn data_mut(&mut self) -> &mut Self::Data {
        &mut self.1
    }
    fn vtable(&self) -> VPtr {
        self.0.vtable()
    }
    unsafe fn vtable_mut(&mut self) -> &mut VPtr {
        self.0.vtable_mut()
    }
}

unsafe impl Class for B {
    type _InheritTrait = dyn B_Meta::InheritTrait;

    type VmtPart = BVmt;
    type Vmt = BCombinedVmt;
    type VmtPtr = &'static BCombinedVmt;
    type Layout<VPtr: 'static + Copy> = BLayout<VPtr>;

    #[inline(always)]
    fn base_offset<C: Class + ?Sized>() -> Option<usize> {
        if std::any::TypeId::of::<C>() == std::any::TypeId::of::<B>() {
            Some(0)
        } else if let Some(ofs) = <A as Class>::base_offset::<C>() {
            Some(ofs + 0)
        } else {
            None
        }
    }
}

unsafe impl<C: Class> internal::SubclassOf<B> for internal::SubclassOfWrapper<C> where
    <C as Class>::_InheritTrait: B_Meta::InheritTrait
{
}

pub struct FallbackGenB<Ofs: HasConst<usize>, O: VmtPartGen<B>, F: VmtPartGen<B>>(
    PhantomData<(Ofs, O, F)>,
);
impl<Ofs: HasConst<usize>, O: VmtPartGen<B>, F: VmtPartGen<B>> HasConst<BVmt>
    for FallbackGenB<Ofs, O, F>
{
    const VALUE: BVmt = BVmt {
        virt_b: match <O::ForOffset<Ofs> as HasConst<BVmt>>::VALUE.virt_b {
            Some(fun) => Some(fun),
            None => <F::ForOffset<Ofs> as HasConst<BVmt>>::VALUE.virt_b,
        },
    };
}

unsafe impl<O: VmtPartGen<B>, F: VmtPartGen<B>> VmtPartGen<B> for internal::FallbackVmtGen<O, F> {
    type ForOffset<Ofs: HasConst<usize>> = FallbackGenB<Ofs, O, F>;
}

impl<C: SubclassOf<B>> B_Impl for DynCls<C> {}
impl<C: SubclassOf<B>> B_Impl for Cls<C> {}

impl B {
    pub fn new(base_a: Cls<A>, data: B) -> Cls<B> {
        const VMT: &BCombinedVmt = &B::make_vmt::<ConstUsizeValue<0>, B>();
        unsafe { Cls::from_layout(BLayout(base_a.into_layout().replace_vptr(VMT), data)) }
    }

    pub const fn make_vmt<Ofs: HasConst<usize>, G: B_Meta::HasVmtParts>() -> BCombinedVmt {
        let vmt =
            <<FallbackVmtGen<G, B> as VmtPartGen<B>>::ForOffset<Ofs> as HasConst<BVmt>>::VALUE;
        vmt.assert_implemented();
        BCombinedVmt(A::make_vmt::<AddConst<Ofs, 0>, FallbackVmtGen<G, B>>(), vmt)
    }
}

impl A_Impl for Impl<B> {}
pub struct BPartGen<Ofs: HasConst<usize>>(PhantomData<Ofs>);
impl<Ofs: HasConst<usize>> HasConst<AVmt> for BPartGen<Ofs> {
    const VALUE: AVmt = <A as Class>::VmtPart::default();
}

unsafe impl VmtPartGen<A> for B {
    type ForOffset<Ofs: HasConst<usize>> = BPartGen<Ofs>;
}

impl B_Impl for Impl<B> {
    fn virt_b(&mut self) -> usize {
        42
    }
}
impl<Ofs: HasConst<usize>> HasConst<BVmt> for BPartGen<Ofs> {
    const VALUE: BVmt = {
        unsafe extern "C" fn virt_b<Ofs: HasConst<usize>>(thisptr: &mut u8) -> usize {
            let derived_ptr = (thisptr as *mut u8).sub(Ofs::VALUE);
            let derived: &mut Impl<B> = &mut *FromThinPtr::from_thin_ptr_mut(derived_ptr);
            B_Impl::virt_b(derived)
        }
        type VMT = <B as Class>::VmtPart;
        VMT {
            virt_b: Some(virt_b::<Ofs>),
            ..BVmt::default()
        }
    };
}

unsafe impl VmtPartGen<B> for B {
    type ForOffset<Ofs: HasConst<usize>> = BPartGen<Ofs>;
}

fn test() {
    let a = A::new(A { a_field: 0 });
    let b = B::new(a, B { b_field: 69 });
}

fn test2(var: &Cls<A>) {}

fn test3(var: &DynCls<B>) {
    test2(var.as_ref());
}

mod other_test {
    use bridgeless::bruh::*;

    struct A;

    impl<Cls, TransitiveBase, Path> InternalSubclassOf<A, TransitiveBase, Path> for Wrapper<Cls>
    where
        Cls: DirectSubclassOf<A>,
        A: SubclassOf<TransitiveBase, Path>,
    {
    }

    struct B;

    impl<Cls, TransitiveBase, Path> InternalSubclassOf<B, TransitiveBase, Path> for Wrapper<Cls>
    where
        Cls: DirectSubclassOf<B>,
        B: SubclassOf<TransitiveBase, Path>,
    {
    }

    // impl<Cls, TransitiveBase, Path> SubclassOf<TransitiveBase, DerivedFrom<B, Path>> for Cls
    // where
    //     Cls: DirectSubclassOf<B>,
    //     B: SubclassOf<TransitiveBase, Path>,
    // {
    // }

    struct C;

    impl<Cls, TransitiveBase, Path> InternalSubclassOf<C, TransitiveBase, Path> for Wrapper<Cls>
    where
        Cls: DirectSubclassOf<C>,
        C: SubclassOf<TransitiveBase, Path>,
    {
    }

    // impl<Cls, TransitiveBase, Path> SubclassOf<TransitiveBase, DerivedFrom<C, Path>> for Cls
    // where
    //     Cls: DirectSubclassOf<C>,
    //     C: SubclassOf<TransitiveBase, Path>,
    // {
    // }

    impl DirectSubclassOf<A> for C {}
    impl DirectSubclassOf<B> for C {}

    struct D;

    impl<Cls, TransitiveBase, Path> InternalSubclassOf<D, TransitiveBase, Path> for Wrapper<Cls>
    where
        Cls: DirectSubclassOf<D>,
        D: SubclassOf<TransitiveBase, Path>,
    {
    }

    // impl<Cls, TransitiveBase, Path> SubclassOf<TransitiveBase, DerivedFrom<D, Path>> for Cls
    // where
    //     Cls: DirectSubclassOf<D>,
    //     D: SubclassOf<TransitiveBase, Path>,
    // {
    // }

    impl DirectSubclassOf<C> for D {}
    impl DirectSubclassOf<B> for D {}

    fn test<Cls, P>(a: impl SubclassOf<Cls, P>) {}

    fn test2<Cls>(inst: Cls) {
        test(inst);
    }
}
