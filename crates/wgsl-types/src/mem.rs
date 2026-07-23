//! Memory layout utilities.

use half::f16;
use itertools::Itertools;

use crate::{
    inst::{
        ArrayInstance, AtomicInstance, Instance, LiteralInstance, MatInstance, StructInstance,
        VecInstance,
    },
    ty::{Ty, Type},
};

impl Instance {
    /// Memory representation of host-shareable instances.
    ///
    /// Returns `None` if the type is not host-shareable.
    pub fn to_buffer(&self) -> Option<Vec<u8>> {
        match self {
            Instance::Literal(l) => l.to_buffer(),
            Instance::Struct(s) => s.to_buffer(),
            Instance::Array(a) => a.to_buffer(),
            Instance::Vec(v) => v.to_buffer(),
            Instance::Mat(m) => m.to_buffer(),
            Instance::Ptr(_) => None,
            Instance::Ref(_) => None,
            Instance::Atomic(a) => a.inner().to_buffer(),
            Instance::Deferred(_) => None,
        }
    }

    /// Load an instance from a byte buffer.
    ///
    /// Returns `None` if the type is not host-shareable, or if the buffer is too small.
    /// The buffer can be larger than the type; extra bytes will be ignored.
    pub fn from_buffer(buf: &[u8], ty: &Type) -> Option<Self> {
        match ty {
            Type::Bool => None,
            Type::AbstractInt => None,
            Type::AbstractFloat => None,
            Type::I32 => buf
                .get(..4)?
                .try_into()
                .ok()
                .map(|buf| LiteralInstance::I32(i32::from_le_bytes(buf)).into()),
            Type::U32 => buf
                .get(..4)?
                .try_into()
                .ok()
                .map(|buf| LiteralInstance::U32(u32::from_le_bytes(buf)).into()),
            Type::F32 => buf
                .get(..4)?
                .try_into()
                .ok()
                .map(|buf| LiteralInstance::F32(f32::from_le_bytes(buf)).into()),
            Type::F16 => buf
                .get(..2)?
                .try_into()
                .ok()
                .map(|buf| LiteralInstance::F16(f16::from_le_bytes(buf)).into()),
            #[cfg(feature = "naga-ext")]
            Type::I64 => buf
                .get(..8)?
                .try_into()
                .ok()
                .map(|buf| LiteralInstance::I64(i64::from_le_bytes(buf)).into()),
            #[cfg(feature = "naga-ext")]
            Type::U64 => buf
                .get(..8)?
                .try_into()
                .ok()
                .map(|buf| LiteralInstance::U64(u64::from_le_bytes(buf)).into()),
            #[cfg(feature = "naga-ext")]
            Type::F64 => buf
                .get(..8)?
                .try_into()
                .ok()
                .map(|buf| LiteralInstance::F64(f64::from_le_bytes(buf)).into()),
            Type::Struct(s) => {
                let mut offset = 0;
                let members = s
                    .members
                    .iter()
                    .map(|m| {
                        // handle the specific case of runtime-sized arrays.
                        // they can only be the last member of a struct.
                        let inst = if let Type::Array(_, None) = &m.ty {
                            let buf = buf.get(offset as usize..)?;
                            Instance::from_buffer(buf, &m.ty)?
                        } else {
                            // TODO: handle errors, check valid size...
                            let size = m.size.or_else(|| m.ty.size_of())?;
                            let align = m.align.or_else(|| m.ty.align_of())?;
                            offset = round_up(align, offset);
                            let buf = buf.get(offset as usize..(offset + size) as usize)?;
                            offset += size;
                            Instance::from_buffer(buf, &m.ty)?
                        };
                        Some(inst)
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(StructInstance::new((**s).clone(), members).into())
            }
            Type::Array(ty, Some(n)) => {
                let mut offset = 0;
                let size = ty.size_of()?;
                let stride = round_up(ty.align_of()?, size);
                let mut comps = Vec::new();
                while comps.len() != *n {
                    let buf = buf.get(offset as usize..(offset + size) as usize)?;
                    offset += stride;
                    let inst = Instance::from_buffer(buf, ty)?;
                    comps.push(inst);
                }
                Some(ArrayInstance::new(comps, false).into())
            }
            Type::Array(ty, None) => {
                let mut offset = 0;
                let size = ty.size_of()?;
                let stride = round_up(ty.align_of()?, size);
                let n = buf.len() as u32 / stride;
                if n == 0 {
                    // arrays must not be empty
                    return None;
                }
                let comps = (0..n)
                    .map(|_| {
                        let buf = buf.get(offset as usize..(offset + size) as usize)?;
                        offset += stride;
                        Instance::from_buffer(buf, ty)
                    })
                    .collect::<Option<_>>()?;
                Some(ArrayInstance::new(comps, true).into())
            }
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(_, _) => None,
            Type::Vec(n, ty) => {
                let mut offset = 0;
                let size = ty.size_of()?;
                let comps = (0..*n)
                    .map(|_| {
                        let buf = buf.get(offset as usize..(offset + size) as usize)?;
                        offset += size;
                        Instance::from_buffer(buf, ty)
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(VecInstance::new(comps).into())
            }
            Type::Mat(c, r, ty) => {
                let mut offset = 0;
                let col_ty = Type::Vec(*r, ty.clone());
                let col_size = col_ty.size_of()?;
                let col_off = round_up(col_ty.align_of()?, col_size);
                let cols = (0..*c)
                    .map(|_| {
                        let buf = buf.get(offset as usize..(offset + col_size) as usize)?;
                        offset += col_off;
                        Instance::from_buffer(buf, &col_ty)
                    })
                    .collect::<Option<Vec<_>>>()?;
                Some(MatInstance::from_cols(cols).into())
            }
            Type::Atomic(ty) => {
                let buf = buf.get(..4)?.try_into().ok()?;
                let inst = match &**ty {
                    Type::I32 => LiteralInstance::I32(i32::from_le_bytes(buf)).into(),
                    Type::U32 => LiteralInstance::U32(u32::from_le_bytes(buf)).into(),
                    _ => unreachable!("atomic type must be u32 or i32"),
                };
                Some(AtomicInstance::new(inst).into())
            }
            Type::Ptr(_, _, _)
            | Type::Ref(_, _, _)
            | Type::Texture(_)
            | Type::Sampler(_)
            | Type::Unknown => None,
            #[cfg(feature = "naga-ext")]
            Type::RayQuery(_) | Type::AccelerationStructure(_) => None,
        }
    }
}

impl LiteralInstance {
    /// Memory representation of host-shareable instances.
    ///
    /// Returns `None` if the type is not host-shareable
    fn to_buffer(self) -> Option<Vec<u8>> {
        match self {
            LiteralInstance::Bool(_) => None,
            LiteralInstance::AbstractInt(_) => None,
            LiteralInstance::AbstractFloat(_) => None,
            LiteralInstance::I32(n) => Some(n.to_le_bytes().to_vec()),
            LiteralInstance::U32(n) => Some(n.to_le_bytes().to_vec()),
            LiteralInstance::F32(n) => Some(n.to_le_bytes().to_vec()),
            LiteralInstance::F16(n) => Some(n.to_le_bytes().to_vec()),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::I64(n) => Some(n.to_le_bytes().to_vec()),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::U64(n) => Some(n.to_le_bytes().to_vec()),
            #[cfg(feature = "naga-ext")]
            LiteralInstance::F64(n) => Some(n.to_le_bytes().to_vec()),
        }
    }
}

// TODO: layout
impl StructInstance {
    /// Memory representation of host-shareable instances.
    ///
    /// Returns `None` if the type is not host-shareable.
    fn to_buffer(&self) -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        for (i, (inst, m)) in self.members.iter().zip(&self.ty.members).enumerate() {
            let len = buf.len() as u32;
            let size = m.size.or_else(|| m.ty.min_size_of())?;

            // handle runtime-size arrays as last struct member
            let size = match inst {
                Instance::Array(a) if a.runtime_sized => {
                    (i == self.members.len() - 1).then(|| a.n() as u32 * size)
                }
                _ => Some(size),
            }?;

            let align = m.align.or_else(|| m.ty.align_of())?;
            let off = round_up(align, len);
            if off > len {
                buf.extend((len..off).map(|_| 0));
            }
            let mut bytes = inst.to_buffer()?;
            let bytes_len = bytes.len() as u32;
            if size > bytes_len {
                bytes.extend((bytes_len..size).map(|_| 0));
            }
            buf.extend(bytes);
        }
        Some(buf)
    }
}

impl ArrayInstance {
    /// Memory representation of host-shareable instances.
    ///
    /// Returns `None` if the type is not host-shareable.
    fn to_buffer(&self) -> Option<Vec<u8>> {
        let mut buf = Vec::new();
        let ty = self.inner_ty();
        let size = ty.size_of()?;
        let stride = round_up(ty.align_of()?, size);
        for c in self.iter() {
            buf.extend(c.to_buffer()?);
            if stride > size {
                buf.extend((size..stride).map(|_| 0))
            }
        }
        Some(buf)
    }
}

impl VecInstance {
    /// Memory representation of host-shareable instances.
    ///
    /// Returns `None` if the type is not host-shareable.
    fn to_buffer(&self) -> Option<Vec<u8>> {
        Some(
            self.iter()
                .flat_map(|v| v.to_buffer().unwrap(/* SAFETY: vector elements must be host-shareable */).into_iter())
                .collect_vec(),
        )
    }
}

impl MatInstance {
    /// Memory representation of host-shareable instances.
    ///
    /// Returns `None` if the type is not host-shareable.
    fn to_buffer(&self) -> Option<Vec<u8>> {
        Some(
            self.iter_cols()
                .flat_map(|v| {
                    // SAFETY: vector elements must be host-shareable
                    let mut v_buf = v.to_buffer().unwrap();
                    let len = v_buf.len() as u32;
                    let align = v.ty().align_of().unwrap();
                    if len < align {
                        v_buf.extend((len..align).map(|_| 0));
                    }
                    v_buf.into_iter()
                })
                .collect_vec(),
        )
    }
}

fn round_up(align: u32, size: u32) -> u32 {
    size.div_ceil(align) * align
}

impl Type {
    /// Compute the size of the type.
    ///
    /// Return `None` if the type is not host-shareable, or if it contains a
    /// runtime-sized array. See [`Type::min_size_of`] for runtime-sized arrays.
    ///
    /// Reference: <https://www.w3.org/TR/WGSL/#alignment-and-size>
    pub fn size_of(&self) -> Option<u32> {
        match self {
            Type::Bool => Some(4),
            Type::AbstractInt => None,
            Type::AbstractFloat => None,
            Type::I32 => Some(4),
            Type::U32 => Some(4),
            Type::F32 => Some(4),
            Type::F16 => Some(2),
            #[cfg(feature = "naga-ext")]
            Type::I64 => Some(8),
            #[cfg(feature = "naga-ext")]
            Type::U64 => Some(8),
            #[cfg(feature = "naga-ext")]
            Type::F64 => Some(8),
            Type::Struct(s) => {
                let past_last_mem = s
                    .members
                    .iter()
                    .map(|m| {
                        // TODO: handle errors, check valid size...
                        let size = m.size.or_else(|| m.ty.size_of())?;
                        let align = m.align.or_else(|| m.ty.align_of())?;
                        Some((size, align))
                    })
                    .try_fold(0, |offset, mem| {
                        let (size, align) = mem?;
                        Some(round_up(align, offset) + size)
                    })?;
                Some(round_up(self.align_of()?, past_last_mem))
            }
            Type::Array(ty, Some(n)) => {
                let (size, align) = (ty.size_of()?, ty.align_of()?);
                Some(*n as u32 * round_up(align, size))
            }
            Type::Array(_, None) => None,
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(_, _) => None,
            Type::Vec(n, ty) => {
                let size = ty.size_of()?;
                Some(*n as u32 * size)
            }
            Type::Mat(c, r, ty) => {
                let align = Type::Vec(*r, ty.clone()).align_of()?;
                Some(*c as u32 * align)
            }
            Type::Atomic(_) => Some(4),
            Type::Ptr(_, _, _)
            | Type::Ref(_, _, _)
            | Type::Texture(_)
            | Type::Sampler(_)
            | Type::Unknown => None,
            #[cfg(feature = "naga-ext")]
            Type::RayQuery(_) | Type::AccelerationStructure(_) => None,
        }
    }

    /// Variant of [`Type::size_of`], but for runtime-sized arrays, it returns the minimum
    /// size of the array, i.e. the size of an array with one element.
    pub fn min_size_of(&self) -> Option<u32> {
        match self {
            Type::Array(ty, None) => Some(round_up(ty.align_of()?, ty.size_of()?)),
            // TODO: should we also compute for structs containing a runtime-sized array?
            // This function is only used once, anyway.
            _ => self.size_of(),
        }
    }

    /// Compute the alignment of the type.
    ///
    /// Return `None` if the type is not host-shareable.
    ///
    /// Reference: <https://www.w3.org/TR/WGSL/#alignment-and-size>
    pub fn align_of(&self) -> Option<u32> {
        match self {
            Type::Bool => Some(4),
            Type::AbstractInt => None,
            Type::AbstractFloat => None,
            Type::I32 => Some(4),
            Type::U32 => Some(4),
            Type::F32 => Some(4),
            Type::F16 => Some(2),
            #[cfg(feature = "naga-ext")]
            Type::I64 => Some(8),
            #[cfg(feature = "naga-ext")]
            Type::U64 => Some(8),
            #[cfg(feature = "naga-ext")]
            Type::F64 => Some(8),
            Type::Struct(s) => s
                .members
                .iter()
                // TODO: check valid align attr
                .map(|m| m.align.or_else(|| m.ty.align_of()))
                .try_fold(0, |a, b| Some(a.max(b?))),
            Type::Array(ty, _) => ty.align_of(),
            #[cfg(feature = "naga-ext")]
            Type::BindingArray(_, _) => None,
            Type::Vec(n, ty) => {
                if *n == 3 {
                    match **ty {
                        Type::I32 | Type::U32 | Type::F32 => Some(16),
                        Type::F16 => Some(8),
                        _ => None,
                    }
                } else {
                    self.size_of()
                }
            }
            Type::Mat(_, r, ty) => Type::Vec(*r, ty.clone()).align_of(),
            Type::Atomic(_) => Some(4),
            Type::Ptr(_, _, _)
            | Type::Ref(_, _, _)
            | Type::Texture(_)
            | Type::Sampler(_)
            | Type::Unknown => None,
            #[cfg(feature = "naga-ext")]
            Type::RayQuery(_) | Type::AccelerationStructure(_) => None,
        }
    }
}
