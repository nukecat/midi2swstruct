use crate::optionarray::*;
use std::mem::{size_of, transmute_copy};
use std::marker::PhantomData;
use crate::sequence::multisequence::*;

// Src is the source type.
// Dst is the destination type.
// 1. if Src is u8, then N should be equal to unsigned integer X * Dst.
// 2. if Dst is u8, then N should be equal to Src size * N.
// N size of multisequence, which is made out of src type.
pub struct MultiSequenceView<'a, Src: Copy, Dst: Copy, const STORAGE_N: usize>
where
    [u8; flag_bytes(STORAGE_N)]:,
{
    parent: &'a MultiSequence<Src, STORAGE_N>,
    _marker: PhantomData<Dst>
}

pub trait EqualToZero {}
impl EqualToZero for [(); 0] {}

//-------------------------------//
// IMPLEMENTATION FOR TYPED VIEW //
//-------------------------------//

impl<'a, T: Copy, const STORAGE_N: usize> MultiSequenceView<'a, u8, T, STORAGE_N>
where
    [(); flag_bytes(STORAGE_N)]:,
    [(); STORAGE_N % size_of::<T>()]: EqualToZero,
{
    fn new_typed(parent: &'a MultiSequence<u8, STORAGE_N>) -> Self {
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

        let arr = self.parent.map.get(&time)?;

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

        self.parent
        .map
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

impl<'a, T: Copy, const STORAGE_N: usize> MultiSequenceView<'a, T, u8, STORAGE_N>
where
    [(); flag_bytes(STORAGE_N)]:,
{
    fn new_raw(parent: &'a MultiSequence<T, STORAGE_N>) -> Self {
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

        let arr = self.parent.map.get(&time)?;
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

        self.parent
        .map
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
    use crate::sequence::{multisequence::MultiSequence, multisequenceview::{MultiSequenceView}};

    #[test]
    fn test0() {
        let multisequence = MultiSequence::<u8, 16>::new();
        let typed_view = MultiSequenceView::<_, u16, _>::new_typed(&multisequence);

        typed_view.floor_typed(127, 8);

        _ = typed_view;
    }
}
