use crate::optionarray::*;
use crate::sequence::multisequence::*;
use crate::sequence::sequence::*;

pub struct SequenceView<'a, T, const N: usize>
where
    [(); flag_bytes(N)]:,
{
    pub(crate) parent: &'a MultiSequence<T, N>,
    pub(crate) index: usize,
}

impl<'a, T, const N: usize> SequenceView<'a, T, N>
where
    [(); flag_bytes(N)]:,
    T: Clone
{
    pub fn floor(&self, time: usize) -> T
    where
        T: Clone + Default
    {
        self.parent.floor(time, self.index)
    }

    /// Iterate sparse updates for this sequence
    pub fn iter(&self) -> impl Iterator<Item = (usize, T)> + '_ {
        self.parent
        .map
        .iter()
        .filter_map(move |(&time, arr)| {
            arr.get(self.index).map(|v| (time, v.clone()))
        })
    }
}

impl<'a, T, const N: usize> Into<Sequence<T>> for SequenceView<'a, T, N>
where
    [(); flag_bytes(N)]:,
    T: Clone
{
    fn into(self) -> Sequence<T> {
        let mut seq = Sequence::new();

        // Iterate all entries in the parent multisequence
        for (&time, arr) in &self.parent.map {
            if let Some(value) = arr.get(self.index) {
                seq.insert(time, value.clone());
            }
        }

        seq
    }
}
