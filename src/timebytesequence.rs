use crate::optionarray::{self, OptionArray};
use std::array::from_fn;
use std::collections::BTreeMap;
use std::ops::Not;

pub struct TimeByteSequence {
    map: BTreeMap<usize, u8>
}
impl TimeByteSequence {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    /// Insert or overwrite value at exact time
    pub fn insert(&mut self, time: usize, value: u8) {
        self.map.insert(time, value);
    }

    /// Get value at exact time.
    pub fn get(&self, time: usize) -> Option<u8> {
        self.map.get(&time).copied()
    }

    /// Get value at time (last known value <= time)
    pub fn get_extrapolated(&self, time: usize) -> u8 {
        self.map
        .range(..=time)
        .next_back()
        .map(|(_, v)| *v)
        .unwrap_or(0)
    }

    /// Returns a mutable reference, if value is present.
    pub fn get_mut(&mut self, time: usize) -> Option<&mut u8> {
        self.map.get_mut(&time)
    }

    /// Returns mutable reference to value. If value is not present, inserts default value and returns a mutable reference.
    pub fn get_value_or_default_mut(&mut self, time: usize) -> &mut u8 {
        self.map.entry(time).or_default()
    }

    /// Returns mutable reference to value. If value is not present, inserts new value and returns a mutable reference.
    pub fn get_value_or_insert_mut(&mut self, time: usize, value: u8) -> &mut u8 {
        self.map.entry(time).or_insert(value)
    }

    /// Remove value at exact time
    pub fn remove(&mut self, time: usize) {
        self.map.remove(&time);
    }

    /// Iterate raw events (no timeline logic)
    pub fn iter(&self) -> impl Iterator<Item = (&usize, &u8)> {
        self.map.iter()
    }

    /// Removes entries that are redundant (value equal to previous)
    pub fn optimize(&mut self) {
        let mut last_value: u8 = 0; // default before first entry
        let mut keys_to_remove = Vec::new();

        for (&time, &value) in &self.map {
            if value == last_value {
                keys_to_remove.push(time);
            } else {
                last_value = value;
            }
        }

        for key in keys_to_remove {
            self.map.remove(&key);
        }
    }
}

macro_rules! impl_bitwise_for_time_sequence {
    ($trait:ident, $method:ident, $op:tt) => {
        impl std::ops::$trait for &TimeByteSequence {
            type Output = TimeByteSequence;

            fn $method(self, rhs: Self) -> TimeByteSequence {
                let mut result = TimeByteSequence::new();

                // collect all time keys
                let times: Vec<_> = self
                .map
                .keys()
                .chain(rhs.map.keys())
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();

                for &time in &times {
                    let value = self.get_extrapolated(time) $op rhs.get_extrapolated(time);
                    if value != 0 {
                        result.insert(time, value);
                    }
                }

                result
            }
        }
    };
}

impl_bitwise_for_time_sequence!(BitAnd, bitand, &);
impl_bitwise_for_time_sequence!(BitOr, bitor, |);
impl_bitwise_for_time_sequence!(BitXor, bitxor, ^);

impl Not for &TimeByteSequence {
    type Output = TimeByteSequence;

    fn not(self) -> TimeByteSequence {
        let mut result = TimeByteSequence::new();
        for (&time, &val) in &self.map {
            result.insert(time, !val);
        }
        result
    }
}

macro_rules! impl_bitwise_assign_time_sequence {
    ($trait:ident, $method:ident, $op:tt) => {
        impl std::ops::$trait<&TimeByteSequence> for TimeByteSequence {
            fn $method(&mut self, rhs: &TimeByteSequence) {
                let mut all_times: Vec<usize> = self.map.keys()
                .chain(rhs.map.keys())
                .copied()
                .collect();
                all_times.sort_unstable();
                all_times.dedup();

                for &time in &all_times {
                    let val_self = self.get_extrapolated(time);
                    let val_rhs = rhs.get_extrapolated(time);
                    let result = val_self $op val_rhs;

                    if result != 0 {
                        self.insert(time, result);
                    } else {
                        self.map.remove(&time);
                    }
                }
            }
        }
    };
}

// Implement for &TimeByteSequence
impl_bitwise_assign_time_sequence!(BitOrAssign, bitor_assign, |);
impl_bitwise_assign_time_sequence!(BitAndAssign, bitand_assign, &);
impl_bitwise_assign_time_sequence!(BitXorAssign, bitxor_assign, ^);

pub struct TimeByteSequenceView<'a, const N: usize> {
    parent: &'a TimeByteMultiSequence<N>,
    index: usize,
}

impl<'a, const N: usize> TimeByteSequenceView<'a, N>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    pub fn get(&self, time: usize) -> u8 {
        self.parent.get(time, self.index)
    }

    /// Iterate sparse updates for this sequence
    pub fn iter(&self) -> impl Iterator<Item = (usize, u8)> + '_ {
        self.parent
        .map
        .iter()
        .filter_map(move |(&time, arr)| {
            arr.get(self.index).map(|v| (time, v))
        })
    }
}

impl<'a, const N: usize> Into<TimeByteSequence> for TimeByteSequenceView<'a, N>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    fn into(self) -> TimeByteSequence {
        let mut seq = TimeByteSequence::new();

        // Iterate all entries in the parent multisequence
        for (&time, arr) in &self.parent.map {
            if let Some(value) = arr.get(self.index) {
                seq.insert(time, value);
            }
        }

        seq
    }
}

pub struct TimeByteSequenceMutView<'a, const N: usize> {
    parent: &'a mut TimeByteMultiSequence<N>,
    index: usize,
}

impl<'a, const N: usize> TimeByteSequenceMutView<'a, N>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    /// Get a mutable proxy for the value at `time`.
    /// Returns a `TimeByteValueMut` which allows modifying it.
    pub fn get_mut(&mut self, time: usize) -> TimeByteValueMut<'_, N> {
        TimeByteValueMut {
            parent: self.parent,
            index: self.index,
            time,
        }
    }

    /// Set value at time directly
    pub fn set(&mut self, time: usize, value: u8) {
        self.parent.set(time, self.index, value);
    }

    /// Clear value at time
    pub fn clear(&mut self, time: usize) {
        if let Some(arr) = self.parent.map.get_mut(&time) {
            arr.clear(self.index);
        }
    }
}

/// Proxy type for mutable access
pub struct TimeByteValueMut<'a, const N: usize> {
    parent: &'a mut TimeByteMultiSequence<N>,
    index: usize,
    time: usize,
}

impl<'a, const N: usize> TimeByteValueMut<'a, N>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    /// Read current value (last-known or 0)
    pub fn get(&self) -> u8 {
        self.parent.get(self.time, self.index)
    }

    /// Update value at this time
    pub fn set(&mut self, value: u8) {
        self.parent.set(self.time, self.index, value);
    }

    /// Clear value at this time
    pub fn clear(&mut self) {
        if let Some(arr) = self.parent.map.get_mut(&self.time) {
            arr.clear(self.index);
        }
    }
}

impl<'a, const N: usize> TimeByteValueMut<'a, N>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    /// Add to current value
    pub fn add(&mut self, value: u8) {
        let new_val = self.get().wrapping_add(value);
        self.set(new_val);
    }

    /// Bitwise OR
    pub fn bitor(&mut self, value: u8) {
        let new_val = self.get() | value;
        self.set(new_val);
    }

    /// Bitwise AND
    pub fn bitand(&mut self, value: u8) {
        let new_val = self.get() & value;
        self.set(new_val);
    }

    /// Bitwise XOR
    pub fn bitxor(&mut self, value: u8) {
        let new_val = self.get() ^ value;
        self.set(new_val);
    }

    /// Subtract (wrapping)
    pub fn sub(&mut self, value: u8) {
        let new_val = self.get().wrapping_sub(value);
        self.set(new_val);
    }
}

impl<'a, const N: usize> std::ops::Deref for TimeByteValueMut<'a, N> {
    type Target = u8;

    fn deref(&self) -> &Self::Target {
        // This returns the current value as owned copy
        // Can't return a real reference because OptionalArray doesn't expose a pointer
        panic!("Cannot deref to mutable reference safely without exposing OptionalArray internals. Use get()/set() instead.");
    }
}

impl<'a, const N: usize> std::ops::DerefMut for TimeByteValueMut<'a, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        panic!("Cannot deref_mut safely. Use set() instead.");
    }
}

pub struct TimeByteMultiSequence<const N: usize>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    map: BTreeMap<usize, OptionArray<u8, N>>
}
impl<const N: usize> TimeByteMultiSequence<N>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    /// Insert/update a single sequence value at a time
    pub fn set(&mut self, time: usize, index: usize, value: u8) {
        assert!(index < N);

        let entry = self.map.entry(time).or_insert_with(OptionArray::new);
        entry.set(index, value);
    }

    /// Insert/update multiple sequence values at a time
    pub fn set_many(&mut self, time: usize, values: &[(usize, u8)]) {
        let entry = self.map.entry(time).or_insert_with(OptionArray::new);
        for &(index, value) in values {
            assert!(index < N);
            entry.set(index, value);
        }
    }

    /// Get value of a single sequence at time
    pub fn get(&self, time: usize, index: usize) -> u8 {
        assert!(index < N);

        self.map
        .range(..=time)
        .rev()
        .find_map(|(_, arr)| arr.get(index))
        .unwrap_or(0)
    }

    /// Get all sequence values at time
    pub fn get_all(&self, time: usize) -> [u8; N] {
        let mut result = [0u8; N];

        for (_, arr) in self.map.range(..=time) {
            for i in 0..N {
                if let Some(v) = arr.get(i) {
                    result[i] = v;
                }
            }
        }

        result
    }

    /// Get selected sequence values at time
    pub fn get_many(&self, time: usize, indices: &[usize]) -> Vec<u8> {
        indices.iter().map(|&i| self.get(time, i)).collect()
    }

    /// Iterate timeline entries (raw, sparse)
    pub fn iter(&self) -> impl Iterator<Item = (&usize, &OptionArray<u8, N>)> {
        self.map.iter()
    }

    pub fn sequence(&self, index: usize) -> TimeByteSequenceView<'_, N> {
        assert!(index < N);
        TimeByteSequenceView { parent: self, index }
    }

    pub fn sequence_mut(&mut self, index: usize) -> TimeByteSequenceMutView<N> {
        assert!(index < N);
        TimeByteSequenceMutView { parent: self, index }
    }

    pub fn to_sequence(&self, index: usize) -> TimeByteSequence {
        assert!(index < N);

        let mut seq = TimeByteSequence::new();

        for (&time, arr) in &self.map {
            if let Some(v) = arr.get(index) {
                seq.insert(time, v);
            }
        }

        seq
    }

    pub fn insert_sequence(&mut self, index: usize, seq: &TimeByteSequence) {
        assert!(index < N);

        for (&time, &value) in seq.iter() {
            self.set(time, index, value);
        }
    }

    pub fn replace_sequence(&mut self, index: usize, seq: &TimeByteSequence) {
        assert!(index < N);

        // Clear existing values
        for arr in self.map.values_mut() {
            arr.clear(index);
        }

        self.insert_sequence(index, seq);
    }

    /// Optimizes the multi-sequence by removing redundant values in one pass
    pub fn optimize(&mut self) -> &mut Self {
        // Track last value for each sequence
        let mut last_values = [0u8; N];

        // Collect entries to remove per time key
        let mut keys_to_clear: Vec<(usize, Vec<usize>)> = Vec::new();

        for (&time, arr) in &self.map {
            let mut indices_to_clear = Vec::new();

            for i in 0..N {
                let value = arr.get(i).unwrap_or(0);
                if value == last_values[i] {
                    indices_to_clear.push(i);
                } else {
                    last_values[i] = value;
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
}

impl<const N: usize> Into<[TimeByteSequence; N]> for TimeByteMultiSequence<N>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    fn into(self) -> [TimeByteSequence; N] {
        from_fn(|i| self.to_sequence(i))
    }
}

impl<const N: usize> From<[TimeByteSequence; N]> for TimeByteMultiSequence<N>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    fn from(value: [TimeByteSequence; N]) -> Self {
        let mut multi_sequence = Self::new();

        for (i, sequence) in value.iter().enumerate() {
            multi_sequence.insert_sequence(i, sequence);
        }

        multi_sequence
    }
}

macro_rules! impl_bitwise_for_multi_sequence {
    ($trait:ident, $method:ident, $op:tt) => {
        impl<const N: usize> std::ops::$trait for &TimeByteMultiSequence<N>
        where
            [u8; optionarray::flag_bytes(N)]:,
        {
            type Output = TimeByteMultiSequence<N>;

            fn $method(self, rhs: Self) -> TimeByteMultiSequence<N> {
                let mut result = TimeByteMultiSequence::<N>::new();

                // collect all time keys
                let mut all_times: Vec<usize> =
                self.map.keys().chain(rhs.map.keys()).copied().collect();
                all_times.sort_unstable();
                all_times.dedup();

                // track last known value for each sequence
                let mut last_self = [0u8; N];
                let mut last_rhs = [0u8; N];

                for &time in &all_times {
                    let arr_result = result.map.entry(time).or_insert_with(|| OptionArray::<u8, N>::new());
                    let arr_self = self.map.get(&time);
                    let arr_rhs = rhs.map.get(&time);

                    for i in 0..N {
                        let val_self = arr_self.and_then(|a| a.get(i)).unwrap_or(last_self[i]);
                        let val_rhs = arr_rhs.and_then(|a| a.get(i)).unwrap_or(last_rhs[i]);
                        let value = val_self $op val_rhs;

                        if value != 0 {
                            arr_result.set(i, value);
                        }

                        last_self[i] = val_self;
                        last_rhs[i] = val_rhs;
                    }
                }

                result
            }
        }
    };
}

// Implement BitAnd, BitOr, BitXor
impl_bitwise_for_multi_sequence!(BitAnd, bitand, &);
impl_bitwise_for_multi_sequence!(BitOr, bitor, |);
impl_bitwise_for_multi_sequence!(BitXor, bitxor, ^);

impl<const N: usize> Not for &TimeByteMultiSequence<N>
where
    [u8; optionarray::flag_bytes(N)]:,
{
    type Output = TimeByteMultiSequence<N>;

    fn not(self) -> TimeByteMultiSequence<N> {
        let mut result = TimeByteMultiSequence::<N>::new();
        let mut last_values = [0u8; N];

        let mut all_times: Vec<usize> = self.map.keys().copied().collect();
        all_times.sort_unstable();

        for &time in &all_times {
            let arr_result = result.map.entry(time)
            .or_insert_with(|| OptionArray::<u8, N>::new());
            let arr_self = self.map.get(&time);

            for i in 0..N {
                let val = arr_self.and_then(|a| a.get(i)).unwrap_or(last_values[i]);
                let v = !val;

                if v != 0 {
                    arr_result.set(i, v);
                }

                last_values[i] = val;
            }
        }

        result
    }
}

macro_rules! impl_bitwise_assign_multi_sequence {
    ($trait:ident, $method:ident, $op:tt) => {
        impl<const N: usize> std::ops::$trait<&TimeByteMultiSequence<N>> for TimeByteMultiSequence<N>
        where
            [u8; optionarray::flag_bytes(N)]:,
        {
            fn $method(&mut self, rhs: &TimeByteMultiSequence<N>) {
                let mut last_self = [0u8; N];
                let mut last_rhs = [0u8; N];

                let mut all_times: Vec<usize> = self.map.keys()
                .chain(rhs.map.keys())
                .copied()
                .collect();
                all_times.sort_unstable();
                all_times.dedup();

                for &time in &all_times {
                    let arr_self = self.map.entry(time).or_insert_with(|| OptionArray::<u8, N>::new());
                    let arr_rhs = rhs.map.get(&time);

                    for i in 0..N {
                        let val_self = arr_self.get(i).unwrap_or(last_self[i]);
                        let val_rhs = arr_rhs.and_then(|a| a.get(i)).unwrap_or(last_rhs[i]);
                        let value = val_self $op val_rhs;

                        if value != 0 {
                            arr_self.set(i, value);
                        } else {
                            arr_self.clear(i);
                        }

                        last_self[i] = val_self;
                        last_rhs[i] = val_rhs;
                    }
                }
            }
        }
    };
}

// Implement for &TimeByteMultiSequence
impl_bitwise_assign_multi_sequence!(BitOrAssign, bitor_assign, |);
impl_bitwise_assign_multi_sequence!(BitAndAssign, bitand_assign, &);
impl_bitwise_assign_multi_sequence!(BitXorAssign, bitxor_assign, ^);

pub struct TimeValueSequence<T> {
    map: BTreeMap<usize, T>
}

impl<T> TimeValueSequence<T> {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    /// Insert or overwrite value at exact time
    pub fn insert(&mut self, time: usize, value: T) {
        self.map.insert(time, value);
    }

    /// Returns a mutable reference, if value is present.
    pub fn get_mut(&mut self, time: usize) -> Option<&mut T> {
        self.map.get_mut(&time)
    }

    /// Returns mutable reference to value. If value is not present, inserts new value and returns a mutable reference.
    pub fn get_value_or_insert_mut(&mut self, time: usize, value: T) -> &mut T {
        self.map.entry(time).or_insert(value)
    }

    /// Remove value at exact time
    pub fn remove(&mut self, time: usize) {
        self.map.remove(&time);
    }

    /// Iterate raw events (no timeline logic)
    pub fn iter(&self) -> impl Iterator<Item = (&usize, &T)> {
        self.map.iter()
    }
}

// Helper trait
trait GetValue<T> {
    fn get_value(map: &BTreeMap<usize, T>, time: usize) -> Option<T>;
}

// Implementation for Copy types
impl<T: Copy> GetValue<T> for T {
    fn get_value(map: &BTreeMap<usize, T>, time: usize) -> Option<T> {
        map.get(&time).copied()
    }
}

// Implementation for non-Copy (Clone) types
impl<T: Clone> GetValue<T> for &T {
    fn get_value(map: &BTreeMap<usize, T>, time: usize) -> Option<T> {
        map.get(&time).cloned()
    }
}

// The main method
impl<T> TimeValueSequence<T> {
    pub fn get(&self, time: usize) -> Option<T>
    where
    T: GetValue<T>,
    {
        T::get_value(&self.map, time)
    }
}

impl<T: Copy + Default> TimeValueSequence<T> {
    /// Get value at time (last known value <= time)
    pub fn get_extrapolated(&self, time: usize) -> T {
        self.map
        .range(..=time)
        .next_back()
        .map(|(_, v)| *v)
        .unwrap_or_default()
    }
}

impl<T: Default> TimeValueSequence<T> {
    /// Returns mutable reference to value. If value is not present, inserts default value and returns a mutable reference.
    pub fn get_value_or_default_mut(&mut self, time: usize) -> &mut T {
        self.map.entry(time).or_default()
    }
}

impl<T: std::cmp::PartialEq + Default + Copy> TimeValueSequence<T> {
    /// Removes entries that are redundant (value equal to previous)
    pub fn optimize(&mut self) {
        let mut last_value: T = T::default(); // default before first entry
        let mut keys_to_remove = Vec::new();

        for (&time, &value) in &self.map {
            if value == last_value {
                keys_to_remove.push(time);
            } else {
                last_value = value;
            }
        }

        for key in keys_to_remove {
            self.map.remove(&key);
        }
    }
}

// TimeByteSequence is like timeline, with time keys which has value.
// It should not have any interpolation when accessing values because it's intended for bitwise manipulation.

// TimeByteMultiSequence is like container for multiple TimeByteSequences.
// Structure of this struct would look like this:
//
//  |  Time  |  0  |  1  |  2  |  3  |  4  |  5  |  6  |  7  |  8  |  9  |  ...  |
//  ------------------------------------------------------------------------------
//  |  Seq1  |  9  |  x  |  x  |  1  |  8  |  x  |  x  |  3  |  x  |  x  |  ...  |
//  |  Seq2  |  0  |  2  |  5  |  x  |  x  |  4  |  4  |  x  |  6  |  7  |  ...  |
//
// So, user can ask for two bytes from Seq1 and Seq2 at the timekey 1. Even if Seq1 doesnt store anything at that timeline,
// user would still get value that was on previous avaible timeline. User would get [9, 2] in that case.
// If Sequence doesnt have value until that point, we just assume it's equal to 0.
