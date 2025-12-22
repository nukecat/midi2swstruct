use bool_traits::*;
use option_array::{self, *};
use std::array::from_fn;
use std::collections::BTreeMap;
use crate::sequence::*;
use crate::sequenceview::*;
use std::ops::{Deref, DerefMut};
use crate::multisequenceview::*;

pub struct MultiSequence<T, const N: usize>(BTreeMap<usize, OptionArray<T, N>>)
where [(); option_array::flag_bytes(N)]:;

trait Type<T> {}
impl<T> Type<T> for T {}

pub trait MultiSequenceLike<T, const N: usize>
where [(); option_array::flag_bytes(N)]: {
    fn get(&self, time: usize) -> Option<&OptionArray<T, N>>;

    fn floor(&self, time: usize) -> [T; N]
    where   T: Clone + Default;

    fn sequence<'a>(&'a self, index: usize) -> SequenceView<'a, T, N>;
}

pub trait RawMultiSequenceLike<const N: usize>
where [(); option_array::flag_bytes(N)]: {
    fn view_as<'a, Dst>(&'a self) -> TORMultiSequenceView<'a, Dst, N>
    where   Dst: Copy,
            Boolean<{N % size_of::<Dst>() == 0}>: IsTrue,
            Boolean<{N == 0}>: IsFalse;

    fn as_sequence<'a, Dst>(&'a self) -> SequenceView<'a, Dst, N>
    where   Dst: Copy,
            Boolean<{N == size_of::<Dst>()}>: IsTrue,
            Boolean<{N % size_of::<Dst>() == 0}>: IsTrue,
            Boolean<{N == 0}>: IsFalse;
}

pub trait TypedMultiSequenceLike<T, const N: usize>
where [(); option_array::flag_bytes(N)]: {
    fn view_raw<'a>(&'a self) -> ROTMultiSequenceView<'a, T, N>
    where   T: Copy;
}

pub trait MultiSequenceLikeMut<T, const N: usize>
where [(); option_array::flag_bytes(N)]: {
    fn insert(&mut self, time: usize, value: T);

    fn change<F>(&mut self, time: usize, f: F)
    where   F: Fn([T; N]) -> [T; N];

    fn sequence_mut<'a>(&'a mut self, index: usize) -> SequenceViewMut<'a, T, N>;
}

pub trait RawMultiSequenceLikeMut<const N: usize>
where [(); option_array::flag_bytes(N)]: {
    fn view_as_mut<'a, Dst>(&'a mut self) -> TORMultiSequenceViewMut<'a, Dst, N>
    where   Dst: Copy,
            Boolean<{N % size_of::<Dst>() == 0}>: IsTrue,
            Boolean<{N == 0}>: IsFalse;

    fn as_sequence_mut<'a, Dst>(&'a self) -> SequenceViewMut<'a, Dst, N>
    where   Dst: Copy,
            Boolean<{N == size_of::<Dst>()}>: IsTrue;
}

trait TypedMultiSequenceLikeMut<T, const N: usize>
where [(); option_array::flag_bytes(N)]: {
    fn view_raw_mut<'a>(&'a mut self) -> ROTMultiSequenceViewMut<'a, T, N>
    where   T: Copy;
}

impl<T, const N: usize> MultiSequenceLike<T, N> for MultiSequence<T, N>
where [(); option_array::flag_bytes(N)]: {
    fn get(&self, time: usize) -> Option<&OptionArray<T, N>> {
        self.0.get(&time)
    }

    fn floor(&self, time: usize) -> [T; N]
    where   T: Clone + Default {
        let mut result: [T; N] = from_fn(|_| T::default());

        if let Some((_key, array)) = self.0.range(..=time).next_back() {
            for i in 0..N {
                if let Some(value) = array.get(i) {
                    result[i] = value.clone();
                }
            }
        }

        result
    }

    fn sequence<'a>(&'a self, index: usize) -> SequenceView<'a, T, N> {
        SequenceView::new(self, index)
    }
}

impl<const N: usize> RawMultiSequenceLike<N> for MultiSequence<u8, N>
where [(); option_array::flag_bytes(N)]: {
    fn view_as<'a, Dst>(&'a self) -> TORMultiSequenceView<'a, Dst, N>
    where   Dst: Copy,
            Boolean<{N % size_of::<Dst>() == 0}>: IsTrue,
            Boolean<{N == 0}>: IsFalse {
        MultiSequenceView::new_typed(self)
    }

    fn as_sequence<'a, Dst>(&'a self) -> SequenceView<'a, Dst, N>
    where   Dst: Copy,
            Boolean<{N == size_of::<Dst>()}>: IsTrue,
            Boolean<{N % size_of::<Dst>() == 0}>: IsTrue,
            Boolean<{N == 0}>: IsFalse, {
        MultiSequenceView::<'a, u8, Dst, N>::new_typed(self).sequence(0)
    }
}

impl<T, const N: usize> TypedMultiSequenceLike<T, N> for MultiSequence<T, N>
where [(); option_array::flag_bytes(N)]: {
    fn view_raw<'a>(&'a self) -> ROTMultiSequenceView<'a, T, N>
    where   T: Copy {
        MultiSequenceView::new_raw(self)
    }
}

impl<T, const N: usize> MultiSequenceLikeMut<T, N> for MultiSequence<T, N> {
    fn insert(&mut self, time: usize, value: T) {

    }

    fn change<F>(&mut self, time: usize, f: F)
    where   F: Fn([T; N]) -> [T; N] {

    }

    fn sequence_mut<'a>(&'a mut self, index: usize) -> SequenceViewMut<'a, T, N> {

    }

    fn view_raw_mut<'a>(&'a mut self) -> MultiSequenceViewMut<'a, T, u8, N>
    where   T: Copy {

    }

    fn view_as_mut<'a, Dst>(&'a mut self) -> MultiSequenceViewMut<'a, T, Dst, N>
    where   T: Type<u8> + Copy,
            Dst: Copy,
            Boolean<{N % size_of::<T>() == 0}>: IsTrue,
            Boolean<{N == 0}>: IsFalse {

    }

    fn as_sequence_mut<'a, Dst>(&'a self) -> SequenceViewMut<'a, Dst, N>
    where   T: Type<u8> + Copy,
            Dst: Copy,
            Boolean<{N == size_of::<Dst>()}>: IsTrue {

    }
}

impl<T, const N: usize> MultiSequence<T, N>
where
    [(); flag_bytes(N)]:,
{
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Insert/update multiple sequence values at a time
    pub fn set_group(&mut self, time: usize, values: &[(usize, T)])
    where
        T: Clone
    {
        let entry = self.entry(time).or_insert_with(OptionArray::new);
        for (index, value) in values {
            assert!(*index < N);
            entry.set(*index, value.clone());
        }
    }

    /// Get value of a single sequence at time
    pub fn floor(&self, time: usize, index: usize) -> T
    where
        T: Clone + Default
    {
        assert!(index < N);

        self
            .range(..=time)
            .rev()
            .find_map(|(_, arr)| arr.get(index).cloned())
            .unwrap_or_default()
    }

    /// Get selected sequence values at time
    pub fn floor_selected(&self, time: usize, indices: &[usize]) -> Vec<T>
    where
        T: Clone + Default
    {
        indices.iter().map(|&i| self.floor(time, i)).collect()
    }

    pub fn sequence(&self, index: usize) -> SequenceView<'_, T, N> {
        assert!(index < N);
        SequenceView { parent: self, index }
    }

    pub fn insert_sequence(&mut self, index: usize, seq: &Sequence<T>)
    where
        T: Clone
    {
        assert!(index < N);

        for (&time, value) in seq.iter() {
            self.set(time, index, value.clone());
        }
    }

    pub fn replace_sequence(&mut self, index: usize, seq: &Sequence<T>)
    where
        T: Clone
    {
        assert!(index < N);

        // Clear existing values
        for arr in self.values_mut() {
            arr.clear(index);
        }

        self.insert_sequence(index, seq);
    }

    /// Optimizes the multi-sequence by removing redundant values in one pass
    pub fn optimize(&mut self) -> &mut Self
    where T: PartialEq {
        // Track last value for each sequence as references
        let mut last_values: [Option<&T>; N] = [(); N].map(|_| None);

        // Collect entries to remove per time key
        let mut keys_to_clear: Vec<(usize, Vec<usize>)> = Vec::new();

        for (&time, arr) in self.iter() {
            let mut indices_to_clear = Vec::new();

            for i in 0..N {
                let value_opt = arr.get(i); // Option<&T>

                if value_opt == last_values[i] {
                    // same as previous, mark for clearing
                    indices_to_clear.push(i);
                } else {
                    last_values[i] = value_opt; // update last seen
                }
            }

            if !indices_to_clear.is_empty() {
                keys_to_clear.push((time, indices_to_clear));
            }
        }

        // Clear redundant values
        for (time, indices) in keys_to_clear {
            if let Some(arr) = self.get_mut(&time) {
                for &i in &indices {
                    arr.clear(i);
                }
            }
        }

        self
    }

    /// Insert/update a single sequence value at a time
    pub fn set(&mut self, time: usize, index: usize, value: T) {
        assert!(index < N);

        let entry = self.entry(time).or_insert_with(OptionArray::new);
        entry.set(index, value);
    }

    pub fn view_raw<'a>(&'a self) -> MultiSequenceView<'a, T, u8, N>
    where T: Copy {
        MultiSequenceView::new_raw(self)
    }
}

impl<const N: usize> MultiSequence<u8, N>
where [(); flag_bytes(N)]:, {
    pub fn view_as<'a, T>(&'a self) ->MultiSequenceView<'a, u8, T, N>
    where
        T: Copy,
        Boolean<{N % size_of::<T>() == 0}>: IsTrue,
        Boolean<{N == 0}>: IsFalse,
    {
        MultiSequenceView::new_typed(self)
    }
}

impl<T, const N: usize> From<MultiSequence<u8, { N * size_of::<T>() }>> for MultiSequence<T, N>
where
    T: Copy,
    [(); flag_bytes(N)]:,
    [(); flag_bytes(N * size_of::<T>())]:,
{
    fn from(raw: MultiSequence<u8, { N * size_of::<T>() }>) -> Self {
        let mut result = MultiSequence::<T, N>::new();

        for (&time, raw_arr) in raw.iter() {
            let mut typed_arr = OptionArray::<T, N>::new();

            for i in 0..N {
                // Each T occupies size_of::<T>() u8s in the raw array
                let mut bytes: [u8; size_of::<T>()] = [0; size_of::<T>()];
                let mut all_present = true;

                for j in 0..size_of::<T>() {
                    let idx = i * size_of::<T>() + j;
                    match raw_arr.get(idx) {
                        Some(&b) => bytes[j] = b,
                        None => {
                            all_present = false;
                            break;
                        }
                    }
                }

                if all_present {
                    // SAFETY: bytes length = size_of::<T>(), T: Copy
                    let value = unsafe { *(bytes.as_ptr() as *const T) };
                    typed_arr.set(i, value);
                }
            }

            result.insert(time, typed_arr);
        }

        result
    }
}


impl<T, const N: usize> From<MultiSequence<T, N>> for MultiSequence<u8, { N * size_of::<T>() }>
where
    T: Copy,
    [(); flag_bytes(N)]:,
    [(); flag_bytes(N * size_of::<T>())]:,
{
    fn from(typed: MultiSequence<T, N>) -> Self {
        let mut result = MultiSequence::<u8, { N * size_of::<T>() }>::new();

        for (&time, typed_arr) in typed.iter() {
            let mut raw_arr = OptionArray::<u8, { N * size_of::<T>() }>::new();

            for i in 0..N {
                if let Some(&value) = typed_arr.get(i) {
                    // Convert T -> bytes
                    let bytes: [u8; size_of::<T>()] =
                    unsafe { std::mem::transmute_copy(&value) };

                    for j in 0..size_of::<T>() {
                        let idx = i * size_of::<T>() + j;
                        raw_arr.set(idx, bytes[j]);
                    }
                }
            }

            result.insert(time, raw_arr);
        }

        result
    }
}
