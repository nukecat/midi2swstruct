use std::array::from_fn;
use std::marker::PhantomData;
use std::mem::MaybeUninit;

pub const fn flag_bytes(n: usize) -> usize {
    (n + 7) / 8
}

pub struct OptionArray<T, const N: usize>
where
    [(); flag_bytes(N)]:,
{
    flags: [u8; flag_bytes(N)],
    data: [MaybeUninit<T>; N],
    _marker: PhantomData<T>
}

impl<T, const N: usize> OptionArray<T, N>
where
    [(); flag_bytes(N)]:,
{
    /// Create a new empty OptionArray
    pub fn new() -> Self {
        let data: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        Self {
            flags: [0; flag_bytes(N)],
            data,
            _marker: PhantomData,
        }
    }

    pub fn len(&self) -> usize {
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
        if self.is_present(index) {
            Some(unsafe { &*self.data[index].as_ptr() })
        } else {
            None
        }
    }

    /// Get mutable reference to value at index, initializing if absent
    pub fn get_or_init(&mut self, index: usize, default: T) -> &mut T {
        assert!(index < N);
        let byte = index / 8;
        let bit = index % 8;

        if !self.is_present(index) {
            self.data[index].write(default);
            self.flags[byte] |= 1 << bit;
        }

        unsafe { &mut *self.data[index].as_mut_ptr() }
    }

    /// Set value at index
    pub fn set(&mut self, index: usize, value: T) {
        assert!(index < N);
        self.data[index].write(value);
        let byte = index / 8;
        let bit = index % 8;
        self.flags[byte] |= 1 << bit;
    }

    /// Clear value at index
    pub fn clear(&mut self, index: usize) {
        assert!(index < N);
        let byte = index / 8;
        let bit = index % 8;
        self.flags[byte] &= !(1 << bit);
    }

    /// Iterate over all entries as Option<&T>
    pub fn iter(&self) -> impl Iterator<Item = Option<&T>> + '_ {
        (0..N).map(move |i| self.get(i))
    }

    /// Fill all entries with a value
    pub fn fill(&mut self, value: T)
    where
        T: Copy,
    {
        for i in 0..N {
            self.set(i, value);
        }
    }
}

impl<T, const N: usize> Into<[Option<T>; N]> for OptionArray<T, N>
where
    [(); flag_bytes(N)]:,
    T: Clone
{
    fn into(self) -> [Option<T>; N] {
        from_fn(|i| self.get(i).cloned())
    }
}

impl<T, const N: usize> From<[Option<T>; N]> for OptionArray<T, N>
where
    [(); flag_bytes(N)]:,
    T: Clone
{
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
            _marker: PhantomData,
        }
    }
}
