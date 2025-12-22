#![feature(generic_const_exprs)]

use std::array::from_fn;
use std::mem::MaybeUninit;

pub const fn flag_bytes(n: usize) -> usize {
    (n + 7) / 8
}

pub struct OptionArray<T, const N: usize>
where [(); flag_bytes(N)]:, {
    flags: [u8; flag_bytes(N)],
    data: [MaybeUninit<T>; N],
}

pub struct IterMut<'a, T, const N: usize>
where [(); flag_bytes(N)]:, {
    array: &'a mut OptionArray<T, N>,
    index: usize,
}

impl<'a, T, const N: usize> Iterator for IterMut<'a, T, N>
where [(); flag_bytes(N)]:, {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < N {
            let i = self.index;
            self.index += 1;
            if self.array.is_present(i) {
                return Some(unsafe { &mut *self.array.data[i].as_mut_ptr() });
            }
        }
        None
    }
}

pub struct IterAllMut<'a, T, const N: usize>
where [(); flag_bytes(N)]:, {
    array: &'a mut OptionArray<T, N>,
    index: usize,
}

impl<'a, T, const N: usize> Iterator for IterAllMut<'a, T, N>
where [(); flag_bytes(N)]:, {
    type Item = Option<&'a mut T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= N {
            return None;
        }

        let i = self.index;
        self.index += 1;

        Some(if self.array.is_present(i) {
            Some(unsafe { &mut *self.array.data[i].as_mut_ptr() })
        } else {
            None
        })
    }
}

impl<T, const N: usize> OptionArray<T, N>
where [(); flag_bytes(N)]:, {
    fn set_flag(&mut self, index: usize, present: bool) {
        assert!(index < N);
        let byte_index = index / 8;
        let bit_index = index % 8;
        if present {
            self.flags[byte_index] |= 1 << bit_index;
        } else {
            self.flags[byte_index] &= !(1 << bit_index);
        }
    }

    /// Create a new empty OptionArray
    pub fn new() -> Self {
        let data: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        Self {
            flags: [0; flag_bytes(N)],
            data
        }
    }

    pub fn len(&self) -> usize {
        let full_bytes = N / 8;
        let mut count = 0;

        // Count full bytes
        for b in &self.flags[..full_bytes] {
            count += b.count_ones() as usize;
        }

        // Count last partial byte
        let rem_bits = N % 8;
        if rem_bits != 0 {
            let last_byte = self.flags[full_bytes] & ((1 << rem_bits) - 1);
            count += last_byte.count_ones() as usize;
        }

        count
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    /// Check if the value at index is present
    pub fn is_present(&self, index: usize) -> bool {
        assert!(index < N);
        let byte = index / 8;
        let bit = index % 8;
        (self.flags[byte] & (1 << bit)) != 0
    }

    /// Get the value at index, if present
    pub fn get(&self, index: usize) -> Option<&T> {
        assert!(index < N);
        if self.is_present(index) {
            Some(unsafe { self.data[index].assume_init_ref() })
        } else {
            None
        }
    }

    /// Get mutable reference to value at index, if present.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        assert!(index < N);
        if self.is_present(index) {
            Some(unsafe { self.data[index].assume_init_mut() })
        } else {
            None
        }
    }

    /// Get mutable reference to value at index, initializing if absent
    pub fn get_or_insert(&mut self, index: usize, value: T) -> &mut T {
        assert!(index < N);
        self.set_flag(index, true);
        self.data[index].write(value)
    }

    /// Get mutable reference to value at index, initializing if absent, using func.
    pub fn get_or_insert_with<F>(&mut self, index: usize, f: F) -> &mut T
    where F: FnOnce() -> T {
        assert!(index < N);
        if !self.is_present(index) {
            self.data[index] = MaybeUninit::new(f());
            self.set_flag(index, true);
        }
        unsafe { self.data[index].assume_init_mut() }
    }

    /// Set value at index
    pub fn insert(&mut self, index: usize, value: T) {
        assert!(index < N);
        if self.is_present(index) {
            unsafe { self.data[index].assume_init_drop() };
        }
        self.data[index] = MaybeUninit::new(value);
        self.set_flag(index, true);
    }

    /// Remove value at index
    pub fn remove(&mut self, index: usize) -> Option<T> {
        assert!(index < N);
        if !self.is_present(index) { return None }
        self.set_flag(index, false);
        Some(unsafe { self.data[index].assume_init_read() })
    }

    pub fn clear(&mut self) {
        for i in 0..N {
            if self.is_present(i) {
                unsafe { self.data[i].assume_init_drop() };
            }
        }
        self.flags.fill(0);
    }

    /// Iterate over all existing entries as T&
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        (0..N).filter_map(move |i| self.get(i))
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T, N> {
        IterMut {
            array: self,
            index: 0,
        }
    }

    pub fn iter_all(&self) -> impl Iterator<Item = Option<&T>> + '_ {
        (0..N).map(move |i| self.get(i))
    }

    pub fn iter_all_mut(&mut self) -> IterAllMut<'_, T, N> {
        IterAllMut {
            array: self,
            index: 0
        }
    }

    /// Fill all entries with a value
    pub fn fill(&mut self, value: T)
    where   T: Copy {
        for i in 0..N {
            if self.is_present(i) {
                unsafe { self.data[i].assume_init_drop() };
            }
            self.data[i] = MaybeUninit::new(value);
        }
        self.flags.fill(0xFF);
    }
}

impl<T, const N: usize> Drop for OptionArray<T, N>
where [(); flag_bytes(N)]:, {
    fn drop(&mut self) {
        self.clear();
    }
}

impl<T, const N: usize> Into<[Option<T>; N]> for OptionArray<T, N>
where [(); flag_bytes(N)]:, T: Clone {
    fn into(self) -> [Option<T>; N] {
        from_fn(|i| self.get(i).cloned())
    }
}

impl<T, const N: usize> From<[Option<T>; N]> for OptionArray<T, N>
where [(); flag_bytes(N)]:, T: Clone {
    fn from(value: [Option<T>; N]) -> Self {
        // SAFETY: An array of MaybeUninit<T> is always valid
        let mut data: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut flags = [0u8; flag_bytes(N)];

        for i in 0..N {
            if let Some(v) = &value[i] {
                data[i].write(v.clone()); // clone T if necessary
                let byte = i / 8;
                let bit = i % 8;
                flags[byte] |= 1 << bit; // mark as initialized
            }
        }

        Self {
            flags,
            data,
        }
    }
}
