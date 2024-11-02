#![no_std]

use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

//TODO: pub use bridgeless_proc_macros::class;

pub mod internal;

use internal::FromThinPtr;

/// Metadata trait specifying the layout of a C++ class without virtual bases.
///
/// # SAFETY
/// **This trait should not be implemented manually**. Invariants that
/// guarantee soundness are not stable and may change subtly between versions.
pub unsafe trait Class: 'static + Sized {
    /// Helper associated type for powering the `SubclassOf` magic.
    type _SubclassOfHelper: ?Sized;

    /// The full class layout, FFI-compatible with the equivalent C++ object.
    ///
    /// `V` is the main (first) vtable.
    type Layout<V: 'static>: 'static + AsRef<Self> + AsMut<Self>;

    /// Type of the main vtable (the one at offset 0) of the class.
    type Vmt: 'static + Copy;

    /// GAT used internally to build virtual function tables at compile-time.
    ///
    /// By having the resulting type implement both a blanket `BaseVtableFor` impl
    /// pointing to the base class's vtable and a blanket non-trait impl providing
    /// the derived vtables, we have DIY specialization!
    type VmtSource<T, const OFFSET: usize>
    where
        T: 'static + internal::FromThinPtr + ?Sized;

    /// If `C` is a base class of `Self` (i.e. `Self: SubclassOf<C>`), returns the
    /// offset of `C`'s layout in `Self::Layout`. Otherwise, returns [`None`].
    ///
    /// Although making this a GAT providing a `const usize` would be preferable, it is not
    /// currently possible without specialization. Instead, it is implemented such that it can
    /// be inlined to a constant when optimizations are applied.
    fn base_offset<C: Class>() -> Option<usize>;
}

/// Trait implemented by certain type configurations of [`Class::VmtSource`] to provide a
/// fallback "specialization" of `C`'s function table that allows for C++ like method inhertiance.
///
/// # SAFETY
/// **This trait should not be implemented manually**. Invariants that
/// guarantee soundness are not stable and may change subtly between versions.
pub unsafe trait BaseVtableFor<C: Class> {
    const VTABLE: C::Vmt;
    const VTABLE_REF: &'static C::Vmt;
}

/// Custom marker trait signifying that a given [`Class`] is an (inclusive) subclass of `B`.
///
/// # SAFETY
/// `Self` must be derived from `C` via the class delcaration macro to implement `SubclassOf<dyn C>`
pub unsafe trait SubclassOf<B: ?Sized>: Class {}

/// Returns the offset of `B`'s layout in `C`'s layout, in bytes.
///
/// While this not a `const fn` due to technical limitations, the optimizer is very good at
/// eliminating the unreachable branches at opt-level > 1, making it effectively constant.
#[inline(always)]
pub fn base_offset<B: Class, C: SubclassOf<B>>() -> usize {
    match C::base_offset::<B>() {
        Some(offset) => offset,
        None => unreachable!(),
    }
}

/// Represents a concrete instance of class `C`. Cannot be a subclass. As such, it can safely
/// implement [`Sized`].
///
/// [`Cls`] can [`DerefMut`] into the [`Class`] type. It also implements [`AsRef`] and
/// [`AsMut`] for all base classes of `C`, so these methods may be used to access base data.
/// Virtual methods of base classes can be called without needing to do this.
#[repr(C)]
pub struct Cls<C: Class>(C::Layout<&'static C::Vmt>);

impl<C: Class> Cls<C> {
    /// Upcast to a base type. The equivalent of `static_cast<B& const>(self)` in C++.
    #[inline(always)]
    pub fn upcast<B: Class>(&self) -> &DynCls<B>
    where
        Self: SubclassOf<B>,
    {
        unsafe {
            let base_thin_ptr = (self as *const _ as *const u8).add(base_offset::<B, Self>());
            &*FromThinPtr::from_thin_ptr(base_thin_ptr)
        }
    }

    /// Upcast to a base type. The equivalent of `static_cast<B&>(self)` in C++.
    #[inline(always)]
    pub fn upcast_mut<B: Class>(&mut self) -> &mut DynCls<B>
    where
        Self: SubclassOf<B>,
    {
        unsafe {
            let base_thin_ptr = (self as *mut _ as *mut u8).add(base_offset::<B, Self>());
            &mut *FromThinPtr::from_thin_ptr_mut(base_thin_ptr)
        }
    }

    /// Convert this instance into its equivalent dynamic type.
    #[inline(always)]
    pub fn as_dyn(&self) -> &DynCls<C> {
        unsafe { &*FromThinPtr::from_thin_ptr(self as *const _ as *const u8) }
    }

    /// Convert this instance into its equivalent dynamic type.
    #[inline(always)]
    pub fn as_dyn_mut(&mut self) -> &mut DynCls<C> {
        unsafe { &mut *FromThinPtr::from_thin_ptr_mut(self as *mut _ as *mut u8) }
    }
}

impl<C: Class> Deref for Cls<C> {
    type Target = C;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
impl<C: Class> DerefMut for Cls<C> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut()
    }
}

impl<B: Class, C: SubclassOf<B>> AsRef<DynCls<B>> for Cls<C> {
    #[inline(always)]
    fn as_ref(&self) -> &DynCls<B> {
        unsafe {
            let base_thin_ptr = (self as *const _ as *const u8).add(base_offset::<B, C>());
            &*FromThinPtr::from_thin_ptr(base_thin_ptr)
        }
    }
}

impl<B: Class, C: SubclassOf<B>> AsMut<DynCls<B>> for Cls<C> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut DynCls<B> {
        unsafe {
            let base_thin_ptr = (self as *mut _ as *mut u8).add(base_offset::<B, C>());
            &mut *FromThinPtr::from_thin_ptr_mut(base_thin_ptr)
        }
    }
}

// We also implement AsRef into a base `Cls`, but only for immutable references.
impl<B: Class, C: SubclassOf<B>> AsRef<Cls<B>> for Cls<C> {
    #[inline(always)]
    fn as_ref(&self) -> &Cls<B> {
        unsafe {
            let base_thin_ptr = (self as *const _ as *const u8).add(base_offset::<B, C>());
            &*(base_thin_ptr as *const _)
        }
    }
}

/// # Important
/// Because it is not [`Sized`], pointers/references to [`DynCls`] are **NOT**
/// ABI-compatible with a [`Cls`] reference or pointer! If any derived class is expected at an FFI
/// boundary, use raw pointers to [`Cls`], [`CRef`] or [`CRefMut`] instead.
///
/// Represents a class derived from `C` ("dynamic" class). May be any derived class.
/// For this reason, it is does not implement [`Sized`].
///
/// [`DynCls`] can [`DerefMut`] into the [`Class`] type. It also implements [`AsRef`] and
/// [`AsMut`] for all base classes of `C`, so these methods may be used to access base data.
/// Virtual methods of base classes can be called without needing to do this.
#[repr(C)]
pub struct DynCls<C: Class>(C::Layout<&'static C::Vmt>, [()]);

impl<C: Class> FromThinPtr for DynCls<C> {
    #[inline(always)]
    unsafe fn from_thin_ptr(ptr: *const u8) -> *const Self {
        core::slice::from_raw_parts(ptr, 0) as *const [u8] as *const Self
    }

    #[inline(always)]
    unsafe fn from_thin_ptr_mut(ptr: *mut u8) -> *mut Self {
        core::slice::from_raw_parts_mut(ptr, 0) as *mut [u8] as *mut Self
    }
}

impl<C: Class> DynCls<C> {
    /// Upcast to a base type. The equivalent of `static_cast<B& const>(self)` in C++.
    #[inline(always)]
    pub fn upcast<B: Class>(&self) -> &DynCls<B>
    where
        Self: SubclassOf<B>,
    {
        unsafe {
            let base_thin_ptr = (self as *const _ as *const u8).add(base_offset::<B, Self>());
            &*FromThinPtr::from_thin_ptr(base_thin_ptr)
        }
    }

    /// Upcast to a base type. The equivalent of `static_cast<B&>(self)` in C++.
    #[inline(always)]
    pub fn upcast_mut<B: Class>(&mut self) -> &mut DynCls<B>
    where
        Self: SubclassOf<B>,
    {
        unsafe {
            let base_thin_ptr = (self as *mut _ as *mut u8).add(base_offset::<B, Self>());
            &mut *FromThinPtr::from_thin_ptr_mut(base_thin_ptr)
        }
    }

    /// Downcast to a derived type. The equivalent of `static_cast<D& const>(self)` in C++.
    ///
    /// # SAFETY
    /// Must be a valid instance of `D`.
    #[inline(always)]
    pub unsafe fn downcast<D: SubclassOf<C>>(&self) -> &DynCls<D> {
        let derived_thin_ptr = (self as *const _ as *const u8).sub(base_offset::<C, D>());
        &*FromThinPtr::from_thin_ptr(derived_thin_ptr)
    }

    /// Downcast to a derived type. The equivalent of `static_cast<D&>(self)` in C++.
    ///
    /// # SAFETY
    /// Must be a valid instance of `D`.
    #[inline(always)]
    pub unsafe fn downcast_mut<D: SubclassOf<C>>(&mut self) -> &mut DynCls<D> {
        let derived_thin_ptr = (self as *mut _ as *mut u8).sub(base_offset::<C, D>());
        &mut *FromThinPtr::from_thin_ptr_mut(derived_thin_ptr)
    }

    /// Transforms `self` into a reference to a concrete class.
    ///
    /// # SAFETY
    /// This is safe as it returns an const reference, so vtables cannot be modified
    /// via [`core::mem::swap`] and such. However, [`Self::as_concrete_mut`] is not.
    #[inline(always)]
    pub fn as_concrete(&self) -> &Cls<C> {
        unsafe { &*(self as *const _ as *const _) }
    }

    /// Transforms `self` into a mutable reference to a concrete class.
    ///
    /// # SAFETY
    /// `self`'s concrete type must be `C`, and not a class derived from `C`.
    #[inline(always)]
    pub unsafe fn as_concrete_mut(&mut self) -> &mut Cls<C> {
        unsafe { &mut *(self as *mut _ as *mut _) }
    }
}

impl<C: Class> Deref for DynCls<C> {
    type Target = C;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}
impl<C: Class> DerefMut for DynCls<C> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut()
    }
}

impl<B: Class, C: SubclassOf<B>> AsRef<DynCls<B>> for DynCls<C> {
    #[inline(always)]
    fn as_ref(&self) -> &DynCls<B> {
        unsafe {
            let base_thin_ptr = (self as *const _ as *const u8).add(base_offset::<B, C>());
            &*FromThinPtr::from_thin_ptr(base_thin_ptr)
        }
    }
}

impl<B: Class, C: SubclassOf<B>> AsMut<DynCls<B>> for DynCls<C> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut DynCls<B> {
        unsafe {
            let base_thin_ptr = (self as *mut _ as *mut u8).add(base_offset::<B, C>());
            &mut *FromThinPtr::from_thin_ptr_mut(base_thin_ptr)
        }
    }
}

// We also allow reference conversions to `Cls` for immutable references
impl<B: Class, C: SubclassOf<B>> AsRef<Cls<B>> for DynCls<C> {
    #[inline(always)]
    fn as_ref(&self) -> &Cls<B> {
        self.as_concrete().as_ref()
    }
}

mod cref_sealed {
    pub trait Mutability: 'static {
        type Variance<'a>;
    }

    pub struct Ref;
    pub struct Mut;

    impl Mutability for Ref {
        type Variance<'a> = &'a Self;
    }
    impl Mutability for Mut {
        type Variance<'a> = &'a mut Self;
    }
}
use cref_sealed::{Mut, Mutability, Ref};

/// Wrapper around a constant reference to a (potentially derived) instance of `C` which is
/// ABI-compatible with a `Cls<C>` reference or [`NonNull`] pointer.
///
/// In particular, `Option<&Cls<C>>` has the same ABI as `Option<CRef<'_, C>>>`.
#[repr(transparent)]
#[allow(private_bounds)]
pub struct CRef<'a, C: Class, A: Mutability = Ref>(NonNull<Cls<C>>, PhantomData<A::Variance<'a>>);
pub type CRefMut<'a, C> = CRef<'a, C, Mut>;

// immutable references can be copied
impl<'a, C: Class> Clone for CRef<'a, C> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}
impl<'a, C: Class> Copy for CRef<'a, C> {}

// CRef `Deref` into a DynCls
impl<'a, C: Class, A: Mutability> Deref for CRef<'a, C, A> {
    type Target = DynCls<C>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        (unsafe { self.0.as_ref() }).as_dyn()
    }
}
// CRefMut `DerefMut` into a DynCls
impl<'a, C: Class> DerefMut for CRefMut<'a, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        (unsafe { self.0.as_mut() }).as_dyn_mut()
    }
}

// Conversions to `CRef` from references to owned types
impl<'a, C: Class, D: SubclassOf<C>> From<&'a Cls<D>> for CRef<'a, C> {
    #[inline(always)]
    fn from(value: &'a Cls<D>) -> Self {
        CRef(
            unsafe { NonNull::new_unchecked(value.as_ref() as *const Cls<C> as *mut _) },
            PhantomData,
        )
    }
}
impl<'a, C: Class, D: SubclassOf<C>> From<&'a DynCls<D>> for CRef<'a, C> {
    #[inline(always)]
    fn from(value: &'a DynCls<D>) -> Self {
        CRef(
            unsafe { NonNull::new_unchecked(value.as_ref() as *const Cls<C> as *mut _) },
            PhantomData,
        )
    }
}
impl<'a, C: Class, D: SubclassOf<C>, A: Mutability> From<&'a mut Cls<D>> for CRef<'a, C, A> {
    #[inline(always)]
    fn from(value: &'a mut Cls<D>) -> Self {
        CRef(
            unsafe { NonNull::new_unchecked(value.as_ref() as *const Cls<C> as *mut _) },
            PhantomData,
        )
    }
}
impl<'a, C: Class, D: SubclassOf<C>, A: Mutability> From<&'a mut DynCls<D>> for CRef<'a, C, A> {
    #[inline(always)]
    fn from(value: &'a mut DynCls<D>) -> Self {
        CRef(
            unsafe { NonNull::new_unchecked(value.as_ref() as *const Cls<C> as *mut _) },
            PhantomData,
        )
    }
}

// Conversions from `CRef` to references to owned types
impl<'a, C: Class, D: SubclassOf<C>, A: Mutability> From<CRef<'a, D, A>> for &'a Cls<C> {
    #[inline(always)]
    fn from(value: CRef<'a, D, A>) -> Self {
        unsafe { value.0.as_ref() }.as_ref()
    }
}
impl<'a, C: Class, D: SubclassOf<C>, A: Mutability> From<CRef<'a, D, A>> for &'a DynCls<C> {
    #[inline(always)]
    fn from(value: CRef<'a, D, A>) -> Self {
        unsafe { value.0.as_ref() }.as_ref()
    }
}
impl<'a, C: Class, D: SubclassOf<C>> From<CRefMut<'a, D>> for &'a mut DynCls<C> {
    #[inline(always)]
    fn from(mut value: CRefMut<'a, D>) -> Self {
        unsafe { value.0.as_mut() }.as_mut()
    }
}

// Conversion from `CRefMut<C>` to `CRef<C>`
impl<'a, C: Class> From<CRefMut<'a, C>> for CRef<'a, C> {
    #[inline(always)]
    fn from(value: CRefMut<'a, C>) -> Self {
        CRef(value.0, PhantomData)
    }
}

// "Borrow" method on `CRefMut` to help using it as a normal mutable ref
impl<'a, C: Class> CRefMut<'_, C> {
    /// "Borrows" this mutable reference, simulating the coercion of a &mut T to a &T.
    #[inline(always)]
    pub fn borrow(&self) -> CRef<'_, C> {
        CRef(self.0, PhantomData)
    }
}

// From impls to avoid needing `unsafe` to create raw pointers from `DynCls` references
impl<'a, C: Class, D: SubclassOf<C>> From<&'a DynCls<D>> for *const Cls<C> {
    #[inline(always)]
    fn from(value: &'a DynCls<D>) -> Self {
        value.as_ref() as *const _
    }
}
impl<'a, C: Class, D: SubclassOf<C>> From<&'a mut DynCls<D>> for *mut Cls<C> {
    #[inline(always)]
    fn from(value: &'a mut DynCls<D>) -> Self {
        unsafe { value.as_mut().as_concrete_mut() as *mut _ }
    }
}

/// Represents a thin pointer to a class instance (or any of its subclasses), that is *owned* by
/// whatever struct contains it, but whose memory is not managed by Rust.
///
/// This is particularly useful when interfacing with C code using `std::unique_ptr` or that
/// stores raw pointers to other classes.
///
/// # FFI considerations
/// The pointer backing the [`CBox`] is assumed to be correctly aligned and pointing to an instance.
/// If the underlying pointer might be null, an `Option<CBox<C>>` may be used instead thanks to
/// [`NonNull`]'s option layout optimization.
#[repr(transparent)]
pub struct CBox<C: Class, M: Mutability = Mut>(NonNull<Cls<C>>, PhantomData<M::Variance<'static>>);

/// Variant of [`CBox`] that only provides an immutable view of its data.
pub type CBoxConst<C> = CBox<C, Ref>;

impl<C: Class> CBox<C> {
    /// Creates a [`CBox`] given a [`NonNull`] pointer.
    ///
    /// # Safety
    /// The caller guarantees that `ptr` is well aligned and not dangling, and that it will not be
    /// dropped before the resulting [`CBox`] is.
    #[inline(always)]
    pub unsafe fn from_non_null(ptr: NonNull<Cls<C>>) -> Self {
        CBox(ptr, PhantomData)
    }

    /// Creates a [`CBox`] given a potentially-null mutable pointer.
    ///
    /// # Safety
    /// The caller guarantees that `ptr` is well aligned and not dangling (but may be null), and
    /// that it will not be dropped before the resulting [`CBox`] is.
    #[inline(always)]
    pub unsafe fn from_ptr(ptr: *mut Cls<C>) -> Option<Self> {
        NonNull::new(ptr).map(|p| CBox(p, PhantomData))
    }
}

// CBox derefs into its inner type
impl<C: Class, M: Mutability> Deref for CBox<C, M> {
    type Target = DynCls<C>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { self.0.as_ref() }.as_dyn()
    }
}
impl<C: Class> DerefMut for CBox<C> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.0.as_mut() }.as_dyn_mut()
    }
}

// AsRef/AsMut conversions from CBox to Cls and DynCls
impl<B: Class, C: SubclassOf<B>, M: Mutability> AsRef<DynCls<B>> for CBox<C, M> {
    #[inline(always)]
    fn as_ref(&self) -> &DynCls<B> {
        self.deref().as_ref()
    }
}
impl<B: Class, C: SubclassOf<B>, M: Mutability> AsRef<Cls<B>> for CBox<C, M> {
    #[inline(always)]
    fn as_ref(&self) -> &Cls<B> {
        self.deref().as_ref()
    }
}
impl<B: Class, C: SubclassOf<B>> AsMut<DynCls<B>> for CBox<C> {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut DynCls<B> {
        self.deref_mut().as_mut()
    }
}

/// Marker type used to provide virtual function implementations for a given type. Has the same
/// layout as [`DynCls<C>`].
///
/// [`Impl`] can [`DerefMut`] into the [`Class`] type. However, unlike [`DynCls`] and [`Cls`],
/// it does not implement any virtual function trait. One must explicitly upcast to a [`Impl<Base>`]
/// to execute its logic!
#[repr(C)]
pub struct Impl<C: Class>(DynCls<C>);

impl<C: Class> FromThinPtr for Impl<C> {
    #[inline(always)]
    unsafe fn from_thin_ptr(ptr: *const u8) -> *const Self {
        core::slice::from_raw_parts(ptr, 0) as *const [u8] as *const Self
    }

    #[inline(always)]
    unsafe fn from_thin_ptr_mut(ptr: *mut u8) -> *mut Self {
        core::slice::from_raw_parts_mut(ptr, 0) as *mut [u8] as *mut Self
    }
}

impl<C: Class> Deref for Impl<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
impl<C: Class> DerefMut for Impl<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}

impl<C: Class> Impl<C> {
    /// Upcast to a base type. The equivalent of `static_cast<B& const>(self)` in C++.
    #[inline(always)]
    pub fn upcast<B: Class>(&self) -> &Impl<B>
    where
        Self: SubclassOf<B>,
    {
        unsafe {
            let base_thin_ptr = (self as *const _ as *const u8).add(base_offset::<B, Self>());
            &*FromThinPtr::from_thin_ptr(base_thin_ptr)
        }
    }

    /// Upcast to a base type. The equivalent of `static_cast<B&>(self)` in C++.
    #[inline(always)]
    pub fn upcast_mut<B: Class>(&mut self) -> &mut Impl<B>
    where
        Self: SubclassOf<B>,
    {
        unsafe {
            let base_thin_ptr = (self as *mut _ as *mut u8).add(base_offset::<B, Self>());
            &mut *FromThinPtr::from_thin_ptr_mut(base_thin_ptr)
        }
    }

    /// Convert this instance into its equivalent dynamic type.
    #[inline(always)]
    pub fn as_dyn(&self) -> &DynCls<C> {
        &self.0
    }

    /// Convert this instance into its equivalent dynamic type.
    #[inline(always)]
    pub fn as_dyn_mut(&mut self) -> &mut DynCls<C> {
        &mut self.0
    }
}

pub mod foreign {
    pub trait MyDynTrait {
        type T: ?Sized;
    }

    pub trait TraitOf<S> {}

    pub trait InternalTraitOf<S> {}

    pub struct Wrapper<T>(T);

    impl<T, U> TraitOf<T> for U where Wrapper<U>: InternalTraitOf<T> {}

    //impl<X, T, U> TraitOf<X> for T where (T, U): InternalTraitOf<X, U> {}
}
