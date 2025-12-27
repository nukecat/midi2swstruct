use bool_traits::*;
use option_array::{self, *};
use std::array::from_fn;
use std::collections::BTreeMap;
use crate::sequence::*;
use crate::sequenceview::*;

pub struct MultiSequence<T, const N: usize>(BTreeMap<usize, OptionArray<T, N>>)
where   [(); option_array::flag_bytes(N)]:;

impl<T, const N: usize> MultiSequence<T, N>
where   [(); flag_bytes(N)]:, {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Get value of all sequences at time
    pub fn floor(&self, time: usize) -> [T; N]
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

    pub fn insert(&mut self, time: usize, value: [T; N]) {
        self.0.insert(time, value.into());
    }

    pub fn change<F>(&mut self, time: usize, f: F)
    where   F: Fn([T; N]) -> [T; N], T: Clone + Default {
        self.0.insert(time, f(self.floor(time)).into());
    }

    pub fn sequence(&self, index: usize) -> SequenceView<'_, T, N> {
        assert!(index < N);
        SequenceView::new(self, index)
    }

    pub fn insert_sequence(&mut self, index: usize, seq: &Sequence<T>)
    where   T: Clone {
        assert!(index < N);

        for (&row, value) in seq.iter() {
            let entry = self.0.entry(row).or_insert_with(|| {
                OptionArray::new()
            });

            entry.insert(index, value.clone());
        }
    }

    /// Optimizes the multi-sequence by removing redundant values in one pass
    pub fn optimize(&mut self) -> &mut Self
    where T: PartialEq {
        // Track last value for each sequence as references
        let mut last_values: [Option<&T>; N] = [(); N].map(|_| None);

        // Collect entries to remove per time key
        let mut keys_to_clear: Vec<(usize, Vec<usize>)> = Vec::new();

        for (&time, arr) in self.0.iter() {
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
            if let Some(arr) = self.0.get_mut(&time) {
                for &i in &indices {
                    arr.remove(i);
                }
            }
        }

        self
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

        for (&time, raw_arr) in raw.0.iter() {
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
                    typed_arr.insert(i, value);
                }
            }

            result.0.insert(time, typed_arr);
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

        for (&time, typed_arr) in typed.0.iter() {
            let mut raw_arr = OptionArray::<u8, { N * size_of::<T>() }>::new();

            for i in 0..N {
                if let Some(&value) = typed_arr.get(i) {
                    // Convert T -> bytes
                    let bytes: [u8; size_of::<T>()] =
                    unsafe { std::mem::transmute_copy(&value) };

                    for j in 0..size_of::<T>() {
                        let idx = i * size_of::<T>() + j;
                        raw_arr.insert(idx, bytes[j]);
                    }
                }
            }

            result.0.insert(time, raw_arr);
        }

        result
    }
}
