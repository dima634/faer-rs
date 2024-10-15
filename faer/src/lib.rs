#![allow(non_snake_case)]

use core::{num::NonZero, sync::atomic::AtomicUsize};
use equator::{assert, debug_assert};
use faer_traits::*;

macro_rules! stack_mat {
    ($ctx: expr, $name: ident, $m: expr, $n: expr, $M: expr, $N: expr, $C: ty, $T: ty $(,)?) => {
        let mut __tmp = {
            #[repr(align(64))]
            struct __Col<T, const M: usize>([T; M]);
            struct __Mat<T, const M: usize, const N: usize>([__Col<T, M>; N]);

            core::mem::MaybeUninit::<C::Of<__Mat<T, $M, $N>>>::uninit()
        };
        let __stack = DynStack::new_any(core::slice::from_mut(&mut __tmp));
        let mut $name = unsafe { temp_mat_uninit($ctx, $m, $n, __stack) }.0;
        let mut $name = $name.as_mat_mut();
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __dbg {
    () => {
        std::eprintln!("[{}:{}:{}]", std::file!(), std::line!(), std::column!())
    };
    ($val:expr $(,)?) => {
        match $val {
            tmp => {
                std::eprintln!("[{}:{}:{}] {} = {:10.6?}",
                    std::file!(), std::line!(), std::column!(), std::stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::__dbg!($val)),+,)
    };
}

#[cfg(feature = "perf-warn")]
#[macro_export]
#[doc(hidden)]
macro_rules! __perf_warn {
    ($name: ident) => {{
        #[inline(always)]
        #[allow(non_snake_case)]
        fn $name() -> &'static ::core::sync::atomic::AtomicBool {
            static $name: ::core::sync::atomic::AtomicBool =
                ::core::sync::atomic::AtomicBool::new(false);
            &$name
        }
        ::core::matches!(
            $name().compare_exchange(
                false,
                true,
                ::core::sync::atomic::Ordering::Relaxed,
                ::core::sync::atomic::Ordering::Relaxed,
            ),
            Ok(_)
        )
    }};
}

#[macro_export]
macro_rules! guards {
    ($($ctx: ident),* $(,)?) => {
        $(::generativity::make_guard!($ctx));*
    };
}

#[macro_export]
macro_rules! with_dim {
    ($name: ident, $value: expr $(,)?) => {
        let __val = $value;
        ::generativity::make_guard!($name);
        let $name = $crate::utils::bound::Dim::new(__val, $name);
    };

    ($value: expr $(,)?) => {
        $crate::utils::bound::Dim::new($value, $crate::unique!())
    };
}

#[macro_export]
macro_rules! unique {
    () => {
        unsafe { $crate::hacks::make_guard_pair(&$crate::hacks::Id::new()).1 }
    };
}

#[macro_export]
macro_rules! unique_dim {
    ($n: expr $(,)?) => {
        unsafe { $crate::hacks::make_guard_pair(&$crate::hacks::Id::new()).1 }
    };
}
pub mod utils;

pub mod iter;

pub mod col;
pub mod diag;
pub mod mat;
pub mod perm;
pub mod row;

pub mod linalg;

#[macro_export]
macro_rules! zipped {
    ($head: expr $(,)?) => {
        $crate::linalg::zip::LastEq($crate::linalg::zip::IntoView::into_view($head), ::core::marker::PhantomData)
    };

    ($head: expr, $($tail: expr),* $(,)?) => {
        $crate::linalg::zip::ZipEq::new($crate::linalg::zip::IntoView::into_view($head), $crate::zipped!($($tail,)*))
    };
}

#[macro_export]
macro_rules! unzipped {
    ($head: pat $(,)?) => {
        $crate::linalg::zip::Last($head)
    };

    ($head: pat, $($tail: pat),* $(,)?) => {
        $crate::linalg::zip::Zip($head, $crate::unzipped!($($tail,)*))
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __transpose_impl {
    ([$([$($col:expr),*])*] $($v:expr;)* ) => {
        [$([$($col,)*],)* [$($v,)*]]
    };
    ([$([$($col:expr),*])*] $($v0:expr, $($v:expr),* ;)*) => {
        $crate::__transpose_impl!([$([$($col),*])* [$($v0),*]] $($($v),* ;)*)
    };
}

#[macro_export]
macro_rules! mat {
    () => {
        {
            compile_error!("number of columns in the matrix is ambiguous");
        }
    };

    ($([$($v:expr),* $(,)?] ),* $(,)?) => {
        {
            let data = ::core::mem::ManuallyDrop::new($crate::__transpose_impl!([] $($($v),* ;)*));
            let data = &*data;
            let ncols = data.len();
            let nrows = (*data.get(0).unwrap()).len();

            #[allow(unused_unsafe)]
            unsafe {
                $crate::mat::Mat::from_fn(nrows, ncols, |i, j| ::core::ptr::from_ref(&data[j][i]).read())
            }
        }
    };
}

pub trait Index: faer_traits::Index + seal::Seal {}
impl<T: faer_traits::Index<Signed: seal::Seal> + seal::Seal> Index for T {}

mod seal {
    pub trait Seal {}
    impl<T: faer_traits::Seal> Seal for T {}
    impl Seal for crate::utils::bound::Dim<'_> {}
    impl<I: crate::Index> Seal for crate::utils::bound::Idx<'_, I> {}
    impl<I: crate::Index> Seal for crate::utils::bound::IdxInc<'_, I> {}
    impl<I: crate::Index> Seal for crate::utils::bound::MaybeIdx<'_, I> {}
}

/// Sealed trait for types that can be created from "unbound" values, as long as their
/// struct preconditions are upheld.
pub trait Unbind<I = usize>: Send + Sync + Copy + core::fmt::Debug + seal::Seal {
    /// Create new value.
    /// # Safety
    /// Safety invariants must be upheld.
    unsafe fn new_unbound(idx: I) -> Self;

    /// Returns the unbound value, unconstrained by safety invariants.
    fn unbound(self) -> I;
}

/// Type that can be used to index into a range.
pub type Idx<Dim, I = usize> = <Dim as ShapeIdx>::Idx<I>;
/// Type that can be used to partition a range.
pub type IdxInc<Dim, I = usize> = <Dim as ShapeIdx>::IdxInc<I>;
/// Either an index or a negative value.
pub type MaybeIdx<Dim, I = usize> = <Dim as ShapeIdx>::MaybeIdx<I>;

/// Base trait for [`Shape`].
pub trait ShapeIdx {
    /// Type that can be used to index into a range.
    type Idx<I: Index>: Unbind<I> + Ord + Eq;
    /// Type that can be used to partition a range.
    type IdxInc<I: Index>: Unbind<I> + Ord + Eq + From<Idx<Self, I>>;
    /// Either an index or a negative value.
    type MaybeIdx<I: Index>: Unbind<I::Signed> + Ord + Eq;
}

/// Matrix dimension.
pub trait Shape:
    Unbind
    + Ord
    + ShapeIdx<Idx<usize>: Ord + Eq + PartialOrd<Self>, IdxInc<usize>: Ord + Eq + PartialOrd<Self>>
{
    /// Whether the types involved have any safety invariants.
    const IS_BOUND: bool = true;

    /// Bind the current value using a invariant lifetime guard.
    #[inline]
    fn bind<'n>(self, guard: generativity::Guard<'n>) -> utils::bound::Dim<'n> {
        utils::bound::Dim::new(self.unbound(), guard)
    }

    /// Cast a slice of bound values to unbound values.
    #[inline]
    fn cast_idx_slice<I: Index>(slice: &[Idx<Self, I>]) -> &[I] {
        unsafe { core::slice::from_raw_parts(slice.as_ptr() as _, slice.len()) }
    }

    /// Cast a slice of bound values to unbound values.
    #[inline]
    fn cast_idx_inc_slice<I: Index>(slice: &[IdxInc<Self, I>]) -> &[I] {
        unsafe { core::slice::from_raw_parts(slice.as_ptr() as _, slice.len()) }
    }

    /// Returns the index `0`, which is always valid.
    #[inline(always)]
    fn start() -> IdxInc<Self> {
        unsafe { IdxInc::<Self>::new_unbound(0) }
    }

    /// Returns the incremented value, as an inclusive index.
    #[inline(always)]
    fn next(idx: Idx<Self>) -> IdxInc<Self> {
        unsafe { IdxInc::<Self>::new_unbound(idx.unbound() + 1) }
    }

    /// Returns the last value, equal to the dimension.
    #[inline(always)]
    fn end(self) -> IdxInc<Self> {
        unsafe { IdxInc::<Self>::new_unbound(self.unbound()) }
    }

    /// Checks if the index is valid, returning `Some(_)` in that case.
    #[inline(always)]
    fn idx(self, idx: usize) -> Option<Idx<Self>> {
        if idx < self.unbound() {
            Some(unsafe { Idx::<Self>::new_unbound(idx) })
        } else {
            None
        }
    }

    /// Checks if the index is valid, returning `Some(_)` in that case.
    #[inline(always)]
    fn idx_inc(self, idx: usize) -> Option<IdxInc<Self>> {
        if idx <= self.unbound() {
            Some(unsafe { IdxInc::<Self>::new_unbound(idx) })
        } else {
            None
        }
    }

    /// Checks if the index is valid, and panics otherwise.
    #[inline(always)]
    fn checked_idx(self, idx: usize) -> Idx<Self> {
        equator::assert!(idx < self.unbound());
        unsafe { Idx::<Self>::new_unbound(idx) }
    }

    /// Checks if the index is valid, and panics otherwise.
    #[inline(always)]
    fn checked_idx_inc(self, idx: usize) -> IdxInc<Self> {
        equator::assert!(idx <= self.unbound());
        unsafe { IdxInc::<Self>::new_unbound(idx) }
    }

    /// Assumes the index is valid.
    /// # Safety
    /// The index must be valid.
    #[inline(always)]
    unsafe fn unchecked_idx(self, idx: usize) -> Idx<Self> {
        equator::debug_assert!(idx < self.unbound());
        unsafe { Idx::<Self>::new_unbound(idx) }
    }

    /// Assumes the index is valid.
    /// # Safety
    /// The index must be valid.
    #[inline(always)]
    unsafe fn unchecked_idx_inc(self, idx: usize) -> IdxInc<Self> {
        equator::debug_assert!(idx <= self.unbound());
        unsafe { IdxInc::<Self>::new_unbound(idx) }
    }

    /// Returns an iterator over the indices between `from` and `to`.
    #[inline(always)]
    fn indices(
        from: IdxInc<Self>,
        to: IdxInc<Self>,
    ) -> impl Clone + ExactSizeIterator + DoubleEndedIterator<Item = Idx<Self>> {
        (from.unbound()..to.unbound()).map(
            #[inline(always)]
            |i| unsafe { Idx::<Self>::new_unbound(i) },
        )
    }
}

impl<T: Send + Sync + Copy + core::fmt::Debug + faer_traits::Seal> Unbind<T> for T {
    #[inline(always)]
    unsafe fn new_unbound(idx: T) -> Self {
        idx
    }

    #[inline(always)]
    fn unbound(self) -> T {
        self
    }
}

impl ShapeIdx for usize {
    type Idx<I: Index> = I;
    type IdxInc<I: Index> = I;
    type MaybeIdx<I: Index> = I::Signed;
}
impl Shape for usize {
    const IS_BOUND: bool = false;
}

pub trait Stride: core::fmt::Debug + Copy + Send + Sync + 'static {
    type Rev: Stride<Rev = Self>;
    fn rev(self) -> Self::Rev;

    fn element_stride(self) -> isize;
}

impl Stride for isize {
    type Rev = Self;

    #[inline(always)]
    fn rev(self) -> Self::Rev {
        -self
    }

    #[inline(always)]
    fn element_stride(self) -> isize {
        self
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ContiguousFwd;
#[derive(Copy, Clone, Debug)]
pub struct ContiguousBwd;

impl Stride for ContiguousFwd {
    type Rev = ContiguousBwd;

    #[inline(always)]
    fn rev(self) -> Self::Rev {
        ContiguousBwd
    }
    #[inline(always)]
    fn element_stride(self) -> isize {
        1
    }
}

impl Stride for ContiguousBwd {
    type Rev = ContiguousFwd;

    #[inline(always)]
    fn rev(self) -> Self::Rev {
        ContiguousFwd
    }
    #[inline(always)]
    fn element_stride(self) -> isize {
        -1
    }
}

#[inline]
fn slice_len<C: Container>(slice: C::Of<&[impl Sized]>) -> usize {
    help!(C);
    let mut len = usize::MAX;
    map!(slice, slice, len = Ord::min(len, slice.len()));
    len
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TryReserveError {
    CapacityOverflow,
    AllocError { layout: core::alloc::Layout },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Conj {
    No,
    Yes,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Accum {
    Replace,
    Add,
}

impl Conj {
    #[inline]
    pub const fn compose(self, other: Self) -> Self {
        match (self, other) {
            (Conj::No, Conj::No) => Conj::No,
            (Conj::Yes, Conj::Yes) => Conj::No,
            (Conj::No, Conj::Yes) => Conj::Yes,
            (Conj::Yes, Conj::No) => Conj::Yes,
        }
    }

    #[inline]
    pub const fn get<C: Container, T: ConjUnit>() -> Self {
        if T::IS_CANONICAL && C::IS_CANONICAL {
            Self::No
        } else {
            Self::Yes
        }
    }

    #[inline]
    pub(crate) fn apply<
        C: Container<Canonical: ComplexContainer>,
        T: ConjUnit<Canonical: ComplexField<C::Canonical>>,
    >(
        ctx: &Ctx<C::Canonical, T::Canonical>,
        value: C::Of<&T>,
    ) -> <C::Canonical as Container>::Of<T::Canonical> {
        let value: <C::Canonical as Container>::Of<&T::Canonical> =
            unsafe { core::mem::transmute_copy(&value) };

        if const { matches!(Self::get::<C, T>(), Conj::Yes) } {
            T::Canonical::conj_impl(ctx, value)
        } else {
            T::Canonical::copy_impl(ctx, value)
        }
    }

    #[inline]
    pub(crate) fn apply_val<
        C: Container<Canonical: ComplexContainer>,
        T: ConjUnit<Canonical: ComplexField<C::Canonical>>,
    >(
        ctx: &Ctx<C::Canonical, T::Canonical>,
        value: &C::Of<T>,
    ) -> <C::Canonical as Container>::Of<T::Canonical> {
        help!(C);
        Self::apply::<C, T>(ctx, as_ref!(value))
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Par {
    Seq,
    #[cfg(feature = "rayon")]
    Rayon(NonZero<usize>),
}

impl Par {
    #[inline]
    #[cfg(feature = "rayon")]
    pub fn rayon(nthreads: usize) -> Self {
        if nthreads == 0 {
            Self::Rayon(NonZero::new(rayon::current_num_threads()).unwrap())
        } else {
            Self::Rayon(NonZero::new(nthreads).unwrap())
        }
    }

    #[inline]
    pub fn degree(&self) -> usize {
        utils::thread::parallelism_degree(*self)
    }
}

#[allow(non_camel_case_types)]
pub type c32 = num_complex::Complex32;
#[allow(non_camel_case_types)]
pub type c64 = num_complex::Complex64;

pub use col::{Col, ColGeneric, ColMut, ColMutGeneric, ColRef, ColRefGeneric};
pub use mat::{Mat, MatGeneric, MatMut, MatMutGeneric, MatRef, MatRefGeneric};
pub use row::{Row, RowGeneric, RowMut, RowMutGeneric, RowRef, RowRefGeneric};

#[allow(unused_imports, dead_code)]
mod internal_prelude {
    pub use crate::{
        prelude::{ctx, default},
        variadics::{l, L},
    };

    pub use crate::{
        col::{
            AsColMut, AsColRef, ColGeneric as Col, ColMutGeneric as ColMut, ColRefGeneric as ColRef,
        },
        diag::{DiagGeneric as Diag, DiagMutGeneric as DiagMut, DiagRefGeneric as DiagRef},
        mat::{
            AsMatMut, AsMatRef, MatGeneric as Mat, MatMutGeneric as MatMut, MatRefGeneric as MatRef,
        },
        perm::{Perm, PermRef},
        row::{
            AsRowMut, AsRowRef, RowGeneric as Row, RowMutGeneric as RowMut, RowRefGeneric as RowRef,
        },
    };

    pub use crate::{
        unzipped, zipped, Accum, Conj, ContiguousBwd, ContiguousFwd, Par, Shape, Stride, Unbind,
    };

    pub use unzipped as uz;
    pub use zipped as z;

    pub use crate::utils::{
        bound::{zero, Array, Dim, Idx, IdxInc, MaybeIdx},
        simd::SimdCtx,
    };

    pub use crate::linalg::{self, temp_mat_scratch, temp_mat_uninit};

    pub use faer_macros::{ghost_tree, math};

    pub use faer_traits::{
        help, help2, ComplexContainer, ComplexField, ConjUnit, Container, Ctx, Index,
        RealContainer, RealField, SignedIndex, SimdArch,
    };

    pub use equator::{assert, assert as Assert, debug_assert, debug_assert as DebugAssert};

    pub use dyn_stack::{DynStack, SizeOverflow, StackReq};

    pub use reborrow::*;

    pub use generativity::make_guard;
}

pub mod prelude {
    use super::*;

    pub use faer_traits::Unit;

    pub use col::{Col, ColMut, ColRef};
    pub use mat::{Mat, MatMut, MatRef};
    pub use row::{Row, RowMut, RowRef};

    #[inline]
    pub fn default<Ctx: Default>() -> Ctx {
        Default::default()
    }

    pub use default as ctx;
}

pub struct ScaleGeneric<C: Container, T>(pub C::Of<T>);
pub type Scale<T> = ScaleGeneric<Unit, T>;

#[inline]
pub fn scale<T>(value: T) -> Scale<T> {
    ScaleGeneric(value)
}

/// 0: Disable
/// 1: None
/// n >= 2: Rayon(n - 2)
///
/// default: Rayon(0)
static GLOBAL_PARALLELISM: AtomicUsize = {
    #[cfg(all(not(miri), feature = "rayon"))]
    {
        AtomicUsize::new(2)
    }
    #[cfg(not(all(not(miri), feature = "rayon")))]
    {
        AtomicUsize::new(1)
    }
};

/// Causes functions that access global parallelism settings to panic.
pub fn disable_global_parallelism() {
    GLOBAL_PARALLELISM.store(0, core::sync::atomic::Ordering::Relaxed);
}

/// Sets the global parallelism settings.
pub fn set_global_parallelism(parallelism: Par) {
    let value = match parallelism {
        Par::Seq => 1,
        #[cfg(feature = "rayon")]
        Par::Rayon(n) => n.get().saturating_add(2),
    };
    GLOBAL_PARALLELISM.store(value, core::sync::atomic::Ordering::Relaxed);
}

/// Gets the global parallelism settings.
///
/// # Panics
/// Panics if global parallelism is disabled.
#[track_caller]
pub fn get_global_parallelism() -> Par {
    let value = GLOBAL_PARALLELISM.load(core::sync::atomic::Ordering::Relaxed);
    match value {
        0 => panic!("Global parallelism is disabled."),
        1 => Par::Seq,
        #[cfg(feature = "rayon")]
        n => Par::rayon(n - 2),
        #[cfg(not(feature = "rayon"))]
        _ => unreachable!(),
    }
}

#[doc(hidden)]
pub mod hacks;

pub mod stats;

pub mod variadics;
