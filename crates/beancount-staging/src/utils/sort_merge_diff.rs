use std::cmp::Ordering;

pub struct SortMergeDiff<T, I1, I2, CompareFn> {
    // both in sorted order
    iter1: I1,
    iter2: I2,

    current_1: Option<T>,
    current_2: Option<T>,

    cmp: CompareFn,
}

impl<T, I1, I2, CompareFn> SortMergeDiff<T, I1, I2, CompareFn>
where
    I1: Iterator<Item = T>,
    I2: Iterator<Item = T>,
    CompareFn: Fn(&T, &T) -> Ordering,
{
    /// Both iterators must be sorted with respect to `cmp`
    pub fn new(mut i1: I1, mut i2: I2, cmp: CompareFn) -> Self {
        SortMergeDiff {
            current_1: i1.next(),
            current_2: i2.next(),
            iter1: i1,
            iter2: i2,
            cmp,
        }
    }
}

#[derive(Debug)]
pub enum JoinResult<T> {
    OnlyInFirst(T),
    OnlyInSecond(T),
    InBoth(T, T),
}

impl<T, I1, I2, CompareFn> Iterator for SortMergeDiff<T, I1, I2, CompareFn>
where
    I1: Iterator<Item = T>,
    I2: Iterator<Item = T>,
    CompareFn: Fn(&T, &T) -> Ordering,
{
    type Item = JoinResult<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let advance = match (&self.current_1, &self.current_2) {
            (None, None) => return None,
            (Some(_), None) => Ordering::Less,    // advance left
            (None, Some(_)) => Ordering::Greater, // advance right
            (Some(a), Some(b)) => (self.cmp)(a, b),
        };
        match advance {
            Ordering::Less => {
                let a = std::mem::replace(&mut self.current_1, self.iter1.next()).unwrap();
                Some(JoinResult::OnlyInFirst(a))
            }
            Ordering::Equal => {
                let a = std::mem::replace(&mut self.current_1, self.iter1.next()).unwrap();
                let b = std::mem::replace(&mut self.current_2, self.iter2.next()).unwrap();
                Some(JoinResult::InBoth(a, b))
            }
            Ordering::Greater => {
                let b = std::mem::replace(&mut self.current_2, self.iter2.next()).unwrap();
                Some(JoinResult::OnlyInSecond(b))
            }
        }
    }
}
