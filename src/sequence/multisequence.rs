use crate::optionarray::{self, *};
use std::collections::BTreeMap;
use crate::sequence::sequence::*;
use crate::sequence::sequenceview::*;

pub struct MultiSequence<T, const N: usize>
where
    [(); optionarray::flag_bytes(N)]:,
{
    pub(crate) map: BTreeMap<usize, OptionArray<T, N>>
}

impl<T, const N: usize> MultiSequence<T, N>
where
    [(); optionarray::flag_bytes(N)]:,
{
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    /// Insert/update multiple sequence values at a time
    pub fn set_group(&mut self, time: usize, values: &[(usize, T)])
    where
        T: Clone
    {
        let entry = self.map.entry(time).or_insert_with(OptionArray::new);
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

        self.map
        .range(..=time)
        .rev()
        .find_map(|(_, arr)| arr.get(index).cloned())
        .unwrap_or_default()
    }

    /// Get all sequence values at time
    pub fn get(&self, time: usize) -> Option<&OptionArray<T, N>>
    {
        self.map.get(&time)
    }

    /// Get selected sequence values at time
    pub fn floor_selected(&self, time: usize, indices: &[usize]) -> Vec<T>
    where
        T: Clone + Default
    {
        indices.iter().map(|&i| self.floor(time, i)).collect()
    }

    /// Iterate timeline entries (raw, sparse)
    pub fn iter(&self) -> impl Iterator<Item = (&usize, &OptionArray<T, N>)> {
        self.map.iter()
    }

    pub fn sequence(&self, index: usize) -> SequenceView<'_, T, N> {
        assert!(index < N);
        SequenceView { parent: self, index }
    }

    pub fn get_sequence(&self, index: usize) -> Sequence<T>
    where
        T: Clone
    {
        assert!(index < N);

        let mut seq = Sequence::new();

        for (&time, arr) in &self.map {
            if let Some(v) = arr.get(index) {
                seq.insert(time, v.clone());
            }
        }

        seq
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
        for arr in self.map.values_mut() {
            arr.clear(index);
        }

        self.insert_sequence(index, seq);
    }

    /// Optimizes the multi-sequence by removing redundant values in one pass
    pub fn optimize(&mut self) -> &mut Self
    where
    T: PartialEq,
    {
        // Track last value for each sequence as references
        let mut last_values: [Option<&T>; N] = [(); N].map(|_| None);

        // Collect entries to remove per time key
        let mut keys_to_clear: Vec<(usize, Vec<usize>)> = Vec::new();

        for (&time, arr) in &self.map {
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
            if let Some(arr) = self.map.get_mut(&time) {
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

        let entry = self.map.entry(time).or_insert_with(OptionArray::new);
        entry.set(index, value);
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

        for (&time, raw_arr) in &raw.map {
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

            result.map.insert(time, typed_arr);
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

        for (&time, typed_arr) in &typed.map {
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

            result.map.insert(time, raw_arr);
        }

        result
    }
}
