//! [`Instance`]s of WGSL [`Type`]s.

use std::{
    cell::{Ref, RefCell, RefMut},
    ops::Index,
    rc::Rc,
};

use half::f16;
use itertools::Itertools;

use crate::{
    Error,
    syntax::{AccessMode, AddressSpace},
    ty::{StructType, Ty, Type},
};

type E = Error;

/// Path to a memory view of an instance.
///
/// See [Instance::view]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemView {
    /// View the whole instance.
    Whole,
    /// Access a `struct` member or `vec` component.
    Member(String, Box<MemView>),
    /// Access an `array`, `vec` or `mat` component.
    Index(usize, Box<MemView>),
}

impl MemView {
    pub fn append_member(&mut self, comp: String) {
        match self {
            MemView::Whole => *self = MemView::Member(comp, Box::new(MemView::Whole)),
            MemView::Member(_, v) | MemView::Index(_, v) => v.append_member(comp),
        }
    }
    pub fn append_index(&mut self, index: usize) {
        match self {
            MemView::Whole => *self = MemView::Index(index, Box::new(MemView::Whole)),
            MemView::Member(_, v) | MemView::Index(_, v) => v.append_index(index),
        }
    }
}

/// Instance of a plain type.
///
/// Reference: <https://www.w3.org/TR/WGSL/#plain-types-section>
#[derive(Clone, Debug, PartialEq)]
pub enum Instance {
    Literal(LiteralInstance),
    Struct(StructInstance),
    Array(ArrayInstance),
    Vec(VecInstance),
    #[cfg(feature = "complex")]
    Complex(ComplexInstance),
    #[cfg(feature = "complex")]
    Quat(QuatInstance),
    Mat(MatInstance),
    Ptr(PtrInstance),
    Ref(RefInstance),
    Atomic(AtomicInstance),
    /// For instances that cannot be computed currently, we store the type.
    /// TODO: remove this
    Deferred(Type),
}

impl Instance {
    pub fn unwrap_literal(self) -> LiteralInstance {
        match self {
            Instance::Literal(field_0) => field_0,
            val => panic!("called `Instance::unwrap_literal()` on a `{val}` value"),
        }
    }
    pub fn unwrap_literal_ref(&self) -> &LiteralInstance {
        match self {
            Instance::Literal(field_0) => field_0,
            val => panic!("called `Instance::unwrap_literal_ref()` on a `{val}` value"),
        }
    }
    pub fn unwrap_vec(self) -> VecInstance {
        match self {
            Instance::Vec(field_0) => field_0,
            val => panic!("called `Instance::unwrap_vec()` on a `{val}` value"),
        }
    }
    pub fn unwrap_vec_ref(&self) -> &VecInstance {
        match self {
            Instance::Vec(field_0) => field_0,
            val => panic!("called `Instance::unwrap_vec_ref()` on a `{val}` value"),
        }
    }
    pub fn unwrap_vec_mut(&mut self) -> &mut VecInstance {
        match self {
            Instance::Vec(field_0) => field_0,
            val => panic!("called `Instance::unwrap_vec_mut()` on a `{val}` value"),
        }
    }
    #[cfg(feature = "complex")]
    pub fn unwrap_complex(self) -> ComplexInstance {
        match self {
            Instance::Complex(field_0) => field_0,
            val => panic!("called `Instance::unwrap_complex()` on a `{val}` value"),
        }
    }
    #[cfg(feature = "complex")]
    pub fn unwrap_complex_ref(&self) -> &ComplexInstance {
        match self {
            Instance::Complex(field_0) => field_0,
            val => panic!("called `Instance::unwrap_complex_ref()` on a `{val}` value"),
        }
    }
    #[cfg(feature = "complex")]
    pub fn unwrap_complex_mut(&mut self) -> &mut ComplexInstance {
        match self {
            Instance::Complex(field_0) => field_0,
            val => panic!("called `Instance::unwrap_complex_mut()` on a `{val}` value"),
        }
    }
    #[cfg(feature = "complex")]
    pub fn unwrap_quat(self) -> QuatInstance {
        match self {
            Instance::Quat(field_0) => field_0,
            val => panic!("called `Instance::unwrap_quat()` on a `{val}` value"),
        }
    }
    #[cfg(feature = "complex")]
    pub fn unwrap_quat_ref(&self) -> &QuatInstance {
        match self {
            Instance::Quat(field_0) => field_0,
            val => panic!("called `Instance::unwrap_quat_ref()` on a `{val}` value"),
        }
    }
    #[cfg(feature = "complex")]
    pub fn unwrap_quat_mut(&mut self) -> &mut QuatInstance {
        match self {
            Instance::Quat(field_0) => field_0,
            val => panic!("called `Instance::unwrap_quat_mut()` on a `{val}` value"),
        }
    }
}

macro_rules! from_enum {
    ($target_enum:ident :: $field:ident ( $from:ident )) => {
        impl From<$from> for $target_enum {
            fn from(value: $from) -> Self {
                $target_enum::$field(value)
            }
        }
    };
}

from_enum!(Instance::Literal(LiteralInstance));
from_enum!(Instance::Struct(StructInstance));
from_enum!(Instance::Array(ArrayInstance));
from_enum!(Instance::Vec(VecInstance));
#[cfg(feature = "complex")]
from_enum!(Instance::Complex(ComplexInstance));
#[cfg(feature = "complex")]
from_enum!(Instance::Quat(QuatInstance));
from_enum!(Instance::Mat(MatInstance));
from_enum!(Instance::Ptr(PtrInstance));
from_enum!(Instance::Ref(RefInstance));
from_enum!(Instance::Atomic(AtomicInstance));
from_enum!(Instance::Deferred(Type));

// Transitive `From` implementations.
// They have to be implemented manually unfortunately.

macro_rules! impl_transitive_from {
    ($from:ident => $middle:ident => $into:ident) => {
        impl From<$from> for $into {
            fn from(value: $from) -> Self {
                $into::from($middle::from(value))
            }
        }
    };
}

impl_transitive_from!(bool => LiteralInstance => Instance);
impl_transitive_from!(i64 => LiteralInstance => Instance);
impl_transitive_from!(f64 => LiteralInstance => Instance);
impl_transitive_from!(i32 => LiteralInstance => Instance);
impl_transitive_from!(u32 => LiteralInstance => Instance);
impl_transitive_from!(f32 => LiteralInstance => Instance);

impl Instance {
    /// Get an instance representing a memory view.
    ///
    /// There are two ways to create a memory view:
    /// * Accessing a `struct` component (`struct.member`)
    /// * Indexing an `array`, `vec`, or `mat` (`arr[n]`)
    ///
    /// Reference: <https://www.w3.org/TR/WGSL/#memory-views>
    pub fn view(&self, view: &MemView) -> Result<&Instance, E> {
        match view {
            MemView::Whole => Ok(self),
            MemView::Member(m, v) => match self {
                Instance::Struct(s) => {
                    let inst = s.member(m).ok_or_else(|| E::Component(s.ty(), m.clone()))?;
                    inst.view(v)
                }
                _ => Err(E::Component(self.ty(), m.clone())),
            },
            MemView::Index(i, view) => match self {
                Instance::Array(a) => {
                    let inst = a
                        .components
                        .get(*i)
                        .ok_or(E::OutOfBounds(*i, a.ty(), a.n()))?;
                    inst.view(view)
                }
                Instance::Vec(v) => {
                    let inst = v
                        .components
                        .get(*i)
                        .ok_or(E::OutOfBounds(*i, v.ty(), v.n()))?;
                    inst.view(view)
                }
                #[cfg(feature = "complex")]
                Instance::Complex(c) => {
                    let inst = c.components.get(*i).ok_or(E::OutOfBounds(*i, c.ty(), 2))?;
                    inst.view(view)
                }
                #[cfg(feature = "complex")]
                Instance::Quat(q) => {
                    let inst = q.components.get(*i).ok_or(E::OutOfBounds(*i, q.ty(), 4))?;
                    inst.view(view)
                }
                Instance::Mat(m) => {
                    let inst = m
                        .components
                        .get(*i)
                        .ok_or(E::OutOfBounds(*i, m.ty(), m.c()))?;
                    inst.view(view)
                }
                _ => Err(E::NotIndexable(self.ty())),
            },
        }
    }

    /// Get an instance representing a memory view.
    ///
    /// See [Self::view]
    pub fn view_mut(&mut self, view: &MemView) -> Result<&mut Instance, E> {
        let ty = self.ty();
        match view {
            MemView::Whole => Ok(self),
            MemView::Member(m, v) => match self {
                Instance::Struct(s) => {
                    let inst = s.member_mut(m).ok_or_else(|| E::Component(ty, m.clone()))?;
                    inst.view_mut(v)
                }
                _ => Err(E::Component(ty, m.clone())),
            },
            MemView::Index(i, view) => match self {
                Instance::Array(a) => {
                    let n = a.n();
                    let inst = a.components.get_mut(*i).ok_or(E::OutOfBounds(*i, ty, n))?;
                    inst.view_mut(view)
                }
                Instance::Vec(v) => {
                    let n = v.n();
                    let inst = v.components.get_mut(*i).ok_or(E::OutOfBounds(*i, ty, n))?;
                    inst.view_mut(view)
                }
                #[cfg(feature = "complex")]
                Instance::Complex(c) => {
                    let inst = c.components.get_mut(*i).ok_or(E::OutOfBounds(*i, ty, 2))?;
                    inst.view_mut(view)
                }
                #[cfg(feature = "complex")]
                Instance::Quat(q) => {
                    let inst = q.components.get_mut(*i).ok_or(E::OutOfBounds(*i, ty, 4))?;
                    inst.view_mut(view)
                }
                Instance::Mat(m) => {
                    let c = m.c();
                    let inst = m.components.get_mut(*i).ok_or(E::OutOfBounds(*i, ty, c))?;
                    inst.view_mut(view)
                }
                _ => Err(E::NotIndexable(ty)),
            },
        }
    }

    /// Mutate the instance.
    ///
    /// This is the operation performed by the assignment operator.
    pub fn write(&mut self, value: Instance) -> Result<Instance, E> {
        if value.ty() != self.ty() {
            return Err(E::WriteRefType(value.ty(), self.ty()));
        }
        let old = std::mem::replace(self, value);
        Ok(old)
    }
}

/// Instance of a numeric literal type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiteralInstance {
    Bool(bool),
    AbstractInt(i64),
    AbstractFloat(f64),
    I32(i32),
    U32(u32),
    F32(f32),
    F16(f16),
    #[cfg(feature = "naga-ext")]
    I64(i64), // identity if representable
    #[cfg(feature = "naga-ext")]
    U64(u64), // reinterpretation of bits
    #[cfg(feature = "naga-ext")]
    F64(f64),
}

from_enum!(LiteralInstance::Bool(bool));
from_enum!(LiteralInstance::AbstractInt(i64));
from_enum!(LiteralInstance::AbstractFloat(f64));
from_enum!(LiteralInstance::I32(i32));
from_enum!(LiteralInstance::U32(u32));
from_enum!(LiteralInstance::F32(f32));
from_enum!(LiteralInstance::F16(f16));

impl LiteralInstance {
    pub fn unwrap_bool(self) -> bool {
        match self {
            LiteralInstance::Bool(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_bool()` on a `{val}` value"),
        }
    }

    pub fn unwrap_abstract_int(self) -> i64 {
        match self {
            LiteralInstance::AbstractInt(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_abstract_int()` on a `{val}` value"),
        }
    }
    pub fn unwrap_abstract_float(self) -> f64 {
        match self {
            LiteralInstance::AbstractFloat(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_abstract_float()` on a `{val}` value"),
        }
    }
    pub fn unwrap_i32(self) -> i32 {
        match self {
            LiteralInstance::I32(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_i32()` on a `{val}` value"),
        }
    }
    pub fn unwrap_u32(self) -> u32 {
        match self {
            LiteralInstance::U32(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_u32()` on a `{val}` value"),
        }
    }
    pub fn unwrap_f32(self) -> f32 {
        match self {
            LiteralInstance::F32(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_f32()` on a `{val}` value"),
        }
    }
    pub fn unwrap_f16(self) -> f16 {
        match self {
            LiteralInstance::F16(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_f16()` on a `{val}` value"),
        }
    }
    #[cfg(feature = "naga-ext")]
    pub fn unwrap_i64(self) -> i64 {
        match self {
            LiteralInstance::I64(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_i64()` on a `{val}` value"),
        }
    }
    #[cfg(feature = "naga-ext")]
    pub fn unwrap_u64(self) -> u64 {
        match self {
            LiteralInstance::U64(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_u64()` on a `{val}` value"),
        }
    }
    #[cfg(feature = "naga-ext")]
    pub fn unwrap_f64(self) -> f64 {
        match self {
            LiteralInstance::F64(field_0) => field_0,
            val => panic!("called `LiteralInstance::unwrap_f64()` on a `{val}` value"),
        }
    }
}

/// Instance of a `struct` type.
///
/// Reference: <https://www.w3.org/TR/WGSL/#struct-types>
#[derive(Clone, Debug, PartialEq)]
pub struct StructInstance {
    pub ty: StructType,
    pub members: Vec<Instance>,
}

impl StructInstance {
    /// Create a `struct` instance.
    ///
    /// # Panics
    /// * if there is not the right number of members
    /// * if the members are not of the right type
    pub fn new(ty: StructType, members: Vec<Instance>) -> Self {
        assert_eq!(ty.members.len(), members.len());
        for (m, m_ty) in members.iter().zip(&ty.members) {
            assert_eq!(m_ty.ty, m.ty());
        }

        Self { ty, members }
    }
    /// Get a `struct` member value by name.
    pub fn member(&self, name: &str) -> Option<&Instance> {
        self.members
            .iter()
            .zip(&self.ty.members)
            .find_map(|(inst, m_ty)| (m_ty.name == name).then_some(inst))
    }
    /// Get a `struct` member value by name.
    pub fn member_mut(&mut self, name: &str) -> Option<&mut Instance> {
        self.members
            .iter_mut()
            .zip(&self.ty.members)
            .find_map(|(inst, m_ty)| (m_ty.name == name).then_some(inst))
    }
    // pub fn iter_members(&self) -> impl Iterator<Item = &(String, Instance)> {
    //     self.members.iter()
    // }
}

/// Instance of an `array<T, N>` type.
///
/// Reference: <https://www.w3.org/TR/WGSL/#array-types>
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ArrayInstance {
    components: Vec<Instance>,
    pub runtime_sized: bool,
}

impl ArrayInstance {
    /// Construct an `array`.
    ///
    /// # Panics
    /// * if the components is empty
    /// * if the components are not all the same type
    pub fn new(components: Vec<Instance>, runtime_sized: bool) -> Self {
        assert!(!components.is_empty());
        assert!(components.iter().map(|c| c.ty()).all_equal());
        Self {
            components,
            runtime_sized,
        }
    }
    /// The element count.
    pub fn n(&self) -> usize {
        self.components.len()
    }
    /// Get an element by index.
    pub fn get(&self, i: usize) -> Option<&Instance> {
        self.components.get(i)
    }
    /// Get an element by index.
    pub fn get_mut(&mut self, i: usize) -> Option<&mut Instance> {
        self.components.get_mut(i)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Instance> {
        self.components.iter()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Instance> {
        self.components.iter_mut()
    }
    pub fn as_slice(&self) -> &[Instance] {
        self.components.as_slice()
    }
}

impl IntoIterator for ArrayInstance {
    type Item = Instance;
    type IntoIter = <Vec<Instance> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

/// Instance of a `vecN<T>` type.
///
/// Reference: <https://www.w3.org/TR/WGSL/#vector-types>
#[derive(Clone, Debug, PartialEq)]
pub struct VecInstance {
    components: ArrayInstance,
}

impl VecInstance {
    /// Construct a `vec`.
    ///
    /// # Panics
    /// * if the components length is not [2, 3, 4]
    /// * if the components are not all the same type
    /// * if the type is not a scalar
    pub fn new(components: Vec<Instance>) -> Self {
        assert!((2..=4).contains(&components.len()));
        let components = ArrayInstance::new(components, false);
        assert!(components.inner_ty().is_scalar());
        Self { components }
    }
    /// The component count.
    pub fn n(&self) -> usize {
        self.components.n()
    }
    /// Get a component by index.
    pub fn get(&self, i: usize) -> Option<&Instance> {
        self.components.get(i)
    }
    /// Get a component by index.
    pub fn get_mut(&mut self, i: usize) -> Option<&mut Instance> {
        self.components.get_mut(i)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Instance> {
        self.components.iter()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Instance> {
        self.components.iter_mut()
    }
    pub fn as_slice(&self) -> &[Instance] {
        self.components.as_slice()
    }
}

impl IntoIterator for VecInstance {
    type Item = Instance;
    type IntoIter = <ArrayInstance as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

impl Index<usize> for VecInstance {
    type Output = Instance;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

impl<T: Into<Instance>> From<[T; 2]> for VecInstance {
    fn from(components: [T; 2]) -> Self {
        Self::new(components.map(Into::into).to_vec())
    }
}
impl<T: Into<Instance>> From<[T; 3]> for VecInstance {
    fn from(components: [T; 3]) -> Self {
        Self::new(components.map(Into::into).to_vec())
    }
}
impl<T: Into<Instance>> From<[T; 4]> for VecInstance {
    fn from(components: [T; 4]) -> Self {
        Self::new(components.map(Into::into).to_vec())
    }
}

/// Instance of a `complex<T>` type.
#[cfg(feature = "complex")]
#[derive(Clone, Debug, PartialEq)]
pub struct ComplexInstance {
    components: ArrayInstance,
}

#[cfg(feature = "complex")]
impl ComplexInstance {
    /// Construct a `complex`.
    ///
    /// # Panics
    /// * if the components length is not 2
    /// * if the components are not all the same type
    /// * if the type is not a scalar
    pub fn new(components: Vec<Instance>) -> Self {
        assert_eq!(components.len(), 2);
        let components = ArrayInstance::new(components, false);
        assert!(components.inner_ty().is_scalar());
        Self { components }
    }
    /// Get a component by index.
    pub fn get(&self, i: usize) -> Option<&Instance> {
        self.components.get(i)
    }
    /// Get a component by index.
    pub fn get_mut(&mut self, i: usize) -> Option<&mut Instance> {
        self.components.get_mut(i)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Instance> {
        self.components.iter()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Instance> {
        self.components.iter_mut()
    }
    pub fn as_slice(&self) -> &[Instance] {
        self.components.as_slice()
    }
}

#[cfg(feature = "complex")]
impl IntoIterator for ComplexInstance {
    type Item = Instance;
    type IntoIter = <ArrayInstance as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

#[cfg(feature = "complex")]
impl Index<usize> for ComplexInstance {
    type Output = Instance;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

#[cfg(feature = "complex")]
impl<T: Into<Instance>> From<[T; 2]> for ComplexInstance {
    fn from(components: [T; 2]) -> Self {
        Self::new(components.map(Into::into).to_vec())
    }
}

/// Instance of a `quat<T>` type.
#[cfg(feature = "complex")]
#[derive(Clone, Debug, PartialEq)]
pub struct QuatInstance {
    components: ArrayInstance,
}

#[cfg(feature = "complex")]
impl QuatInstance {
    /// Construct a `quat`.
    ///
    /// # Panics
    /// * if the components length is not 4
    /// * if the components are not all the same type
    /// * if the type is not a scalar
    pub fn new(components: Vec<Instance>) -> Self {
        assert_eq!(components.len(), 4);
        let components = ArrayInstance::new(components, false);
        assert!(components.inner_ty().is_scalar());
        Self { components }
    }
    /// Get a component by index.
    pub fn get(&self, i: usize) -> Option<&Instance> {
        self.components.get(i)
    }
    /// Get a component by index.
    pub fn get_mut(&mut self, i: usize) -> Option<&mut Instance> {
        self.components.get_mut(i)
    }
    pub fn iter(&self) -> impl Iterator<Item = &Instance> {
        self.components.iter()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Instance> {
        self.components.iter_mut()
    }
    pub fn as_slice(&self) -> &[Instance] {
        self.components.as_slice()
    }
}

#[cfg(feature = "complex")]
impl IntoIterator for QuatInstance {
    type Item = Instance;
    type IntoIter = <ArrayInstance as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

#[cfg(feature = "complex")]
impl Index<usize> for QuatInstance {
    type Output = Instance;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).unwrap()
    }
}

#[cfg(feature = "complex")]
impl<T: Into<Instance>> From<[T; 4]> for QuatInstance {
    fn from(components: [T; 4]) -> Self {
        Self::new(components.map(Into::into).to_vec())
    }
}

/// Instance of a `matCxR<T>` type.
///
/// Reference: <https://www.w3.org/TR/WGSL/#matrix-types>
#[derive(Clone, Debug, PartialEq)]
pub struct MatInstance {
    /// Column vectors of the matrix
    components: Vec<Instance>,
}

impl MatInstance {
    /// Construct a `mat` from column vectors.
    ///
    /// # Panics
    /// * if the number of columns is not [2, 3, 4]
    /// * if the columns don't have the same number of rows
    /// * if the number of rows is not [2, 3, 4]
    /// * if the elements don't have the same type
    /// * if the type is not a scalar
    pub fn from_cols(components: Vec<Instance>) -> Self {
        assert!((2..=4).contains(&components.len()));
        assert!(
            components
                .iter()
                .map(|c| c.unwrap_vec_ref().n())
                .all_equal(),
            "MatInstance columns must have the same number for rows"
        );
        assert!(
            components.iter().map(|c| c.ty()).all_equal(),
            "MatInstance columns must have the same type"
        );
        Self { components }
    }

    /// The row count.
    pub fn r(&self) -> usize {
        self.components.first().unwrap().unwrap_vec_ref().n()
    }
    /// The column count.
    pub fn c(&self) -> usize {
        self.components.len()
    }
    /// Get a column vector.
    pub fn col(&self, i: usize) -> Option<&Instance> {
        self.components.get(i)
    }
    /// Get a column vector.
    pub fn col_mut(&mut self, i: usize) -> Option<&mut Instance> {
        self.components.get_mut(i)
    }
    /// Get a component.
    pub fn get(&self, col: usize, row: usize) -> Option<&Instance> {
        self.col(col).and_then(|v| v.unwrap_vec_ref().get(row))
    }
    /// Get a component.
    pub fn get_mut(&mut self, i: usize, j: usize) -> Option<&mut Instance> {
        self.col_mut(i).and_then(|v| v.unwrap_vec_mut().get_mut(j))
    }
    pub fn iter_cols(&self) -> impl Iterator<Item = &Instance> {
        self.components.iter()
    }
    pub fn iter_cols_mut(&mut self) -> impl Iterator<Item = &mut Instance> {
        self.components.iter_mut()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Instance> {
        self.components
            .iter()
            .flat_map(|v| v.unwrap_vec_ref().iter())
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Instance> {
        self.components
            .iter_mut()
            .flat_map(|v| v.unwrap_vec_mut().iter_mut())
    }
}
impl IntoIterator for MatInstance {
    type Item = Instance;
    type IntoIter = <Vec<Instance> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.components.into_iter()
    }
}

/// Instance of a `ptr<AS,T,AM>` type.
///
/// Reference: <https://www.w3.org/TR/WGSL/#ref-ptr-types>
#[derive(Clone, Debug, PartialEq)]
pub struct PtrInstance {
    pub ptr: RefInstance,
}

impl From<RefInstance> for PtrInstance {
    fn from(r: RefInstance) -> Self {
        Self { ptr: r }
    }
}

/// Instance of a `ref<AS,T,AM>` type.
///
/// Reference: <https://www.w3.org/TR/WGSL/#ref-ptr-types>
#[derive(Clone, Debug, PartialEq)]
pub struct RefInstance {
    /// Inner type
    pub ty: Type,
    pub space: AddressSpace,
    pub access: AccessMode,
    pub view: MemView,
    pub ptr: Rc<RefCell<Instance>>,
}

impl RefInstance {
    pub fn new(inst: Instance, space: AddressSpace, access: AccessMode) -> Self {
        let ty = inst.ty();
        Self {
            ty,
            space,
            access,
            view: MemView::Whole,
            ptr: Rc::new(RefCell::new(inst)),
        }
    }
}

impl From<PtrInstance> for RefInstance {
    fn from(p: PtrInstance) -> Self {
        p.ptr
    }
}

impl RefInstance {
    /// Get a reference to a `struct` or `vec` member.
    pub fn view_member(&self, comp: String) -> Result<Self, E> {
        if !self.access.is_read() {
            return Err(E::NotRead);
        }
        let mut view = self.view.clone();
        view.append_member(comp);
        let ty = self.ptr.borrow().view(&view)?.ty();
        Ok(Self {
            ty,
            space: self.space,
            access: self.access,
            view,
            ptr: self.ptr.clone(),
        })
    }
    /// Get a reference to an `array`, `vec` or `mat` component.
    pub fn view_index(&self, index: usize) -> Result<Self, E> {
        if !self.access.is_read() {
            return Err(E::NotRead);
        }
        let mut view = self.view.clone();
        view.append_index(index);
        let ty = self.ptr.borrow().view(&view)?.ty();
        Ok(Self {
            ty,
            space: self.space,
            access: self.access,
            view,
            ptr: self.ptr.clone(),
        })
    }

    pub fn read<'a>(&'a self) -> Result<Ref<'a, Instance>, E> {
        if !self.access.is_read() {
            return Err(E::NotRead);
        }
        Ok(Ref::<'a, Instance>::map(self.ptr.borrow(), |r| {
            r.view(&self.view).expect("invalid reference")
        }))
    }

    pub fn write(&self, value: Instance) -> Result<(), E> {
        if !self.access.is_write() {
            return Err(E::NotWrite);
        }
        if value.ty() != self.ty {
            return Err(E::WriteRefType(value.ty(), self.ty.clone()));
        }
        let mut r = self.ptr.borrow_mut();
        let view = r.view_mut(&self.view).expect("invalid reference");
        assert!(view.ty() == value.ty());
        let _ = std::mem::replace(view, value);
        Ok(())
    }

    pub fn read_write<'a>(&'a self) -> Result<RefMut<'a, Instance>, E> {
        if !self.access.is_write() {
            return Err(E::NotReadWrite);
        }
        Ok(RefMut::<'a, Instance>::map(self.ptr.borrow_mut(), |r| {
            r.view_mut(&self.view).expect("invalid reference")
        }))
    }
}

/// `atomic<T>` Instance.
///
/// Reference: <https://www.w3.org/TR/WGSL/#atomic-types>
#[derive(Clone, Debug, PartialEq)]
pub struct AtomicInstance {
    content: Box<Instance>,
}

impl AtomicInstance {
    /// # Panics
    /// * if the instance is not an i32 or u32
    pub fn new(inst: Instance) -> Self {
        assert!(matches!(inst.ty(), Type::I32 | Type::U32));
        Self {
            content: inst.into(),
        }
    }

    pub fn inner(&self) -> &Instance {
        &self.content
    }

    pub fn inner_mut(&mut self) -> &mut Instance {
        &mut self.content
    }
}
