use std::collections::{BTreeMap, btree_map::Entry};

pub struct Sequence<T> {
    map: BTreeMap<usize, T>
}

impl<T> Sequence<T> {
    pub fn new() -> Self {
        Self {
            map: BTreeMap::new(),
        }
    }

    /// Insert or overwrite value at exact time
    pub fn insert(&mut self, time: usize, value: T) {
        self.map.insert(time, value);
    }

    pub fn get(&self, time: usize) -> Option<&T> {
        self.map.get(&time)
    }

    /// Returns a mutable reference, if value is present.
    pub fn get_mut(&mut self, time: usize) -> Option<&mut T> {
        self.map.get_mut(&time)
    }

    /// Remove value at exact time
    pub fn remove(&mut self, time: usize) {
        self.map.remove(&time);
    }

    /// Iterate raw events (no timeline logic)
    pub fn iter(&self) -> impl Iterator<Item = (&usize, &T)> {
        self.map.iter()
    }

    /// Get value at time (last known value <= time) or default.
    pub fn floor(&self, time: usize) -> T
    where
        T: Clone + Default
    {
        self.map
        .range(..=time)
        .next_back()
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
    }

    /// Removes entries that are redundant (value equal to previous)
    pub fn optimize(&mut self)
    where
        T:  Eq +
            Default +
            Clone
    {
        let mut last_value: T = T::default(); // default before first entry
        let mut keys_to_remove = Vec::new();

        for (&time, value) in &self.map {
            if *value == last_value {
                keys_to_remove.push(time);
            } else {
                last_value = value.clone();
            }
        }

        for key in keys_to_remove {
            self.map.remove(&key);
        }
    }
}

macro_rules! impl_bitwise_for_sequence {
    ($trait:ident, $method:ident, $op:tt) => {
        impl<T> std::ops::$trait for &Sequence<T>
        where
            T:  Clone +
                Default +
                PartialEq +
                std::ops::$trait<Output = T>
        {
            type Output = Sequence<T>;

            fn $method(self, rhs: Self) -> Sequence<T> {
                let mut result = Sequence::new();

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
                    let value = self.floor(time) $op rhs.floor(time);
                    if value != T::default() {
                        result.insert(time, value);
                    }
                }

                result
            }
        }
    };
}

impl_bitwise_for_sequence!(BitAnd, bitand, &);
impl_bitwise_for_sequence!(BitOr, bitor, |);
impl_bitwise_for_sequence!(BitXor, bitxor, ^);

impl<T> std::ops::Not for &Sequence<T>
where
    T: Copy + std::ops::Not<Output = T>,
{
    type Output = Sequence<T>;

    fn not(self) -> Sequence<T> {
        let mut result = Sequence::new();
        for (&time, &val) in &self.map {
            result.insert(time, !val);
        }
        result
    }
}

macro_rules! impl_bitwise_assign_sequence {
    ($trait:ident, $method:ident, $op:tt, $trait2:ident) => {
        impl<T> std::ops::$trait<&Sequence<T>> for Sequence<T>
        where
            T:  Default +
                PartialEq +
                Clone +
                std::ops::$trait2<Output = T>
        {
            fn $method(&mut self, rhs: &Sequence<T>) {
                let mut all_times: Vec<usize> = self.map.keys()
                .chain(rhs.map.keys())
                .copied()
                .collect();
                all_times.sort_unstable();
                all_times.dedup();

                for &time in &all_times {
                    let val_self = self.floor(time);
                    let val_rhs = rhs.floor(time);
                    let result = val_self $op val_rhs;

                    if result != T::default() {
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
impl_bitwise_assign_sequence!(BitOrAssign, bitor_assign, |, BitOr);
impl_bitwise_assign_sequence!(BitAndAssign, bitand_assign, &, BitAnd);
impl_bitwise_assign_sequence!(BitXorAssign, bitxor_assign, ^, BitXor);


