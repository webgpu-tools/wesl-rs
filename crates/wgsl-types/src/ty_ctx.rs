use crate::{
    arena::{Id, UniqueArena},
    ty::StructType,
};
#[derive(Debug, Default)]
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

    pub fn clone_for_error(&self) -> Box<TyContext> {
        Box::new(Self {
            struct_arena: self.struct_arena.clone(),
        })
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

/// Custom display trait that includes a context.
/// Workaround for the orphan rule.
pub trait DisplayWithContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>, context: &TyContext) -> std::fmt::Result;
}

impl<T: DisplayWithContext> std::fmt::Display for WithContext<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        T::fmt(self.value, f, self.context)
    }
}
