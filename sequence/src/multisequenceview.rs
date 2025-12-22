use option_array::*;
use std::mem::{size_of, transmute_copy};
use std::marker::PhantomData;
use crate::multisequence::*;
use bool_traits::*;
use crate::sequenceview::*;

// Src is the source type.
// Dst is the destination type.
// 1. if Src is u8, then N should be equal to unsigned integer X * Dst.
// 2. if Dst is u8, then N should be equal to Src size * N.
// N size of multisequence, which is made out of src type.
pub struct MultiSequenceView<'a, Src, Dst, const STORAGE_N: usize>
where   Src: Copy,
        Dst: Copy,
        [(); flag_bytes(STORAGE_N)]:, {
    parent: &'a MultiSequence<Src, STORAGE_N>,
    _marker: PhantomData<Dst>
}

/// Raw-over-typed multi-sequence view.
pub type ROTMultiSequenceView<'a, Src, const STORAGE_N: usize> = MultiSequenceView<'a, Src, u8, STORAGE_N>;

/// Typed-over-raw multi-sequence view.
pub type TORMultiSequenceView<'a, Dst, const STORAGE_N: usize> = MultiSequenceView<'a, u8, Dst, STORAGE_N>;

//-------------------------------//
// IMPLEMENTATION FOR TYPED VIEW //
//-------------------------------//

impl<'a, T, const STORAGE_N: usize> TORMultiSequenceView<'a, T, STORAGE_N>
where   T: Copy,
        [(); flag_bytes(STORAGE_N)]:,
        Boolean<{STORAGE_N % size_of::<T>() == 0}>: IsTrue,
        Boolean<{STORAGE_N == 0}>: IsFalse {

    pub(crate) fn new_typed(parent: &'a MultiSequence<u8, STORAGE_N>) -> Self {
        Self {
            parent,
            _marker: PhantomData,
        }
    }

    pub fn get_typed(&self, time: usize, index: usize) -> Option<T>
    where
        [(); STORAGE_N % size_of::<T>()]:,
    {
        let elem_size = size_of::<T>();
        let start = index * elem_size;

        if start + elem_size > STORAGE_N {
            return None;
        }

        let arr = self.parent.get(&time)?;

        let mut bytes: [u8; size_of::<T>()] = [0; size_of::<T>()];

        for i in 0..elem_size {
            bytes[i] = *arr.get(start + i)?;
        }

        Some(unsafe { transmute_copy(&bytes) })
    }

    pub fn floor_typed(&self, time: usize, index: usize) -> T
    where
        [(); STORAGE_N % size_of::<T>()]:,
        T: Default
    {
        let elem_size = size_of::<T>();
        let start = index * elem_size;

        if start + elem_size > STORAGE_N {
            panic!("Out of bounds");
        }

        self
        .parent
        .range(..=time)
        .rev()
        .find_map(|(_, arr)| {
            let mut bytes: [u8; size_of::<T>()] = [0; size_of::<T>()];

            for i in 0..elem_size {
                bytes[i] = *arr.get(start + i)?;
            }

            Some(unsafe { transmute_copy(&bytes) })
        })
        .unwrap_or_default()
    }

}

//-----------------------------//
// IMPLEMENTATION FOR RAW VIEW //
//-----------------------------//

impl<'a, T: Copy, const STORAGE_N: usize> ROTMultiSequenceView<'a, T, STORAGE_N>
where
    [(); flag_bytes(STORAGE_N)]:,
{
    pub(crate) fn new_raw(parent: &'a MultiSequence<T, STORAGE_N>) -> Self {
        Self {
            parent,
            _marker: PhantomData,
        }
    }

    pub fn get_raw(&self, time: usize, flat_index: usize) -> Option<u8>
    where
        [(); size_of::<T>()]:,
    {
        let elem_size = size_of::<T>();
        let elem_index = flat_index / elem_size;
        let byte_offset = flat_index % elem_size;

        let arr = self.parent.get(&time)?;
        let value = arr.get(elem_index)?;

        let bytes: [u8; size_of::<T>()] = unsafe { transmute_copy(value) };
        Some(bytes[byte_offset])
    }

    pub fn floor_raw(&self, time: usize, flat_index: usize) -> u8
    where
        [(); size_of::<T>()]:,
    {
        let elem_size = size_of::<T>();
        let elem_index = flat_index / elem_size;
        let byte_offset = flat_index % elem_size;

        self
        .parent
        .range(..=time)
        .rev()
        .find_map(|(_, arr)| {
            let value = arr.get(elem_index)?;
            let bytes: [u8; size_of::<T>()] = unsafe { transmute_copy(value) };
            Some(bytes[byte_offset])
        })
        .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use crate::{multisequence::MultiSequence, multisequenceview::{MultiSequenceView}};

    #[test]
    fn test0() {
        let multi_sequence = MultiSequence::<u64, 16>::new();
        let _raw_view = multi_sequence.view_raw();

        let multi_sequence2 = MultiSequence::<u8, 16>::new();
        let typed_view = multi_sequence2.view_as::<u16>();

        typed_view.floor_typed(127, 8);

        _ = typed_view;
    }
}
