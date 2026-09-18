use crate::{
    arena::{Id, UniqueArena},
    ty::StructType,
};

#[derive(Default)]
pub struct TyContext {
    pub struct_arena: UniqueArena<StructType>,
}

impl TyContext {
    pub fn display<'a, T>(&'a self, value: &'a T) -> WithContext<'a, T> {
        WithContext {
            value,
            context: self,
        }
    }
}

impl std::ops::Index<Id<StructType>> for TyContext {
    type Output = StructType;

    #[inline]
    fn index(&self, id: Id<StructType>) -> &Self::Output {
        &self.struct_arena[id]
    }
}

pub struct WithContext<'a, T> {
    pub value: &'a T,
    pub context: &'a TyContext,
}
