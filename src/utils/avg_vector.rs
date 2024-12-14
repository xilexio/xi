use std::cmp::min;
use std::ops::{Add, Div, Sub};
use num_traits::{AsPrimitive, FromPrimitive, Zero};
use crate::a;
use crate::utils::sampling::{LARGE_SAMPLE_SIZE, SMALL_SAMPLE_SIZE};

/// A FIFO of `N` values, initialized to zero, with their sum maintained in `sum`
/// and a sum of last `M` maintained in `small_sample_sum`.
/// `V` should be large enough for the sum of `n` recent values to not overflow.
// TODO To decrease the memory usage, store already aggregated samples over the small sample.
//      The sum will not be as accurate, but the average will be good enough.
#[derive(Debug, Clone)]
pub struct AvgVector<V, const N: usize = LARGE_SAMPLE_SIZE, const M: usize = SMALL_SAMPLE_SIZE> {
    data: [V; N],
    i: usize,
    pub sum: V,
    pub small_sample_sum: V,
    pub samples: usize,
}

impl<V, const N: usize, const M: usize> AvgVector<V, N, M>
where
    V: Copy + Sub<Output = V> + Add<Output = V>,
{
    pub fn push(&mut self, value: V) {
        self.i = (self.i + 1) % N;
        let replaced_value = self.data[self.i];
        let small_sample_replaced_value = self.data[(self.i + N - M) % N];
        self.data[self.i] = value;
        self.sum = self.sum + value - replaced_value;
        self.small_sample_sum = self.small_sample_sum + value - small_sample_replaced_value;
        self.samples = min(N, self.samples + 1);
    }

    pub fn last(&self) -> V {
        self.data[self.i]
    }
    
    pub fn avg<A>(&self) -> A 
    where
        V: AsPrimitive<A>,
        A: Copy + FromPrimitive + Div<Output = A> + 'static,
        usize: AsPrimitive<A>,
    {
        self.sum.as_() / N.as_()
    }
    
    pub fn small_sample_avg<A>(&self) -> A 
    where
        V: AsPrimitive<A>,
        A: Copy + FromPrimitive + Div<Output = A> + 'static,
        usize: AsPrimitive<A>,
    {
        self.small_sample_sum.as_() / M.as_()
    }
    
    pub fn samples(&self) -> usize {
        N
    }
    
    pub fn small_samples(&self) -> usize {
        M
    }
}

impl<V, const N: usize, const M: usize> Default for AvgVector<V, N, M>
where
    V: Zero + Copy,
{
    fn default() -> Self {
        a!(N > 0);
        a!(M < N);
        Self {
            data: [V::zero(); N],
            i: 0,
            sum: V::zero(),
            small_sample_sum: V::zero(),
            samples: 0,
        }
    }
}