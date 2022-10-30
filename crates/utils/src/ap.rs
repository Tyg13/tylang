#[derive(Debug, PartialEq, Eq)]
pub struct Int {
    storage: IntStorage,
    bit_width: u64,
}

impl Int {
    pub fn small(val: u64, bit_width: u64) -> Self {
        debug_assert!(width(val) <= bit_width);
        Self {
            storage: IntStorage::Small(val),
            bit_width,
        }
    }

    pub fn big(parts: Vec<u64>, bit_width: u64) -> Self {
        debug_assert!(parts.len() >= 2);

        let leading_parts_width = (parts.len() - 1) as u64 * 64;
        let last_part_width = width(*parts.last().unwrap());
        let parts_bit_width = leading_parts_width + last_part_width;
        debug_assert!(parts_bit_width <= bit_width);

        Self {
            storage: IntStorage::Big(parts),
            bit_width,
        }
    }
}

impl std::ops::Add for Int {
    type Output = Int;
    fn add(self, rhs: Self) -> Self::Output {
        let mut bit_width = match (&self.storage, &rhs.storage) {
            (IntStorage::Small(_), IntStorage::Big(_)) => rhs.bit_width,
            (IntStorage::Big(_), IntStorage::Small(_)) => self.bit_width,
            (IntStorage::Small(_), IntStorage::Small(_))
            | (IntStorage::Big(_), IntStorage::Big(_)) => {
                std::cmp::max(self.bit_width, rhs.bit_width)
            }
        };
        match (self.storage, rhs.storage) {
            (IntStorage::Small(v1), IntStorage::Small(v2)) => {
                let (res, carry) = v1.overflowing_add(v2);
                bit_width = std::cmp::max(bit_width, width(res));
                if carry {
                    Int::big(vec![res, 1u64], 65)
                } else {
                    Int::small(res, bit_width)
                }
            }
            (IntStorage::Big(v1), IntStorage::Small(v2)) => {
                add_parts_to_parts(&[v2], &v1, bit_width)
            }
            (IntStorage::Small(v1), IntStorage::Big(v2)) => {
                add_parts_to_parts(&[v1], &v2, bit_width)
            }
            (IntStorage::Big(v1), IntStorage::Big(v2)) => {
                add_parts_to_parts(&v1, &v2, bit_width)
            }
        }
    }
}

fn add_parts_to_parts(v1: &[u64], v2: &[u64], mut bit_width: u64) -> Int {
    let largest_number_of_parts = std::cmp::max(v1.len(), v2.len());
    let smallest_number_of_parts = std::cmp::min(v1.len(), v2.len());
    let mut res = Vec::with_capacity(largest_number_of_parts);

    let mut last_part_had_carry = false;
    for i in 0..smallest_number_of_parts {
        let (mut part, mut carry) = v1[i].overflowing_add(v2[i]);
        if last_part_had_carry {
            let carry_caused_carry;
            (part, carry_caused_carry) = part.overflowing_add(1);
            carry |= carry_caused_carry;
        }
        res.push(part);
        last_part_had_carry = carry;
    }

    let larger_parts = if v1.len() > v2.len() { v1 } else { v2 };
    for i in smallest_number_of_parts..largest_number_of_parts {
        let (part, carry) = if last_part_had_carry {
            larger_parts[i].overflowing_add(1u64)
        } else {
            (larger_parts[i], false)
        };
        res.push(part);
        last_part_had_carry = carry;
    }

    let leading_parts_width = (64 * (res.len() - 1)) as u64;
    let last_part_width = width(*res.last().unwrap());
    bit_width = std::cmp::max(bit_width, leading_parts_width + last_part_width);

    if last_part_had_carry {
        res.push(1u64);
        bit_width = leading_parts_width + 64 + 1;
    }

    Int::big(res, bit_width)
}

impl std::ops::Shr<usize> for Int {
    type Output = Int;

    fn shr(self, mut shift: usize) -> Self::Output {
        if shift == 0 {
            return self;
        }
        match self.storage {
            IntStorage::Small(s) => Int::small(s.shr(shift), self.bit_width),
            IntStorage::Big(mut parts) => {
                let parts_to_drop = shift / 64;
                if parts_to_drop > 0 {
                    shift = shift % 64;
                    parts.rotate_left(parts_to_drop);
                }
                if shift > 0 {
                    let mut last_low_bits = 0;
                    for part in parts.iter_mut().rev() {
                        let low_bits = *part & ((1 << shift) - 1);
                        let shifted = part.shr(shift);
                        *part = shifted | (last_low_bits << (64 - shift));
                        last_low_bits = low_bits;
                    }
                }
                Int::big(parts, self.bit_width)
            }
        }
    }
}

fn extract_high_n_bits(v: u64, bits: usize) -> u64 {
    (v & (u64::MAX << (64 - bits))) >> (64 - bits)
}

impl std::ops::Shl<usize> for Int {
    type Output = Int;

    fn shl(self, shift: usize) -> Self::Output {
        debug_assert!(shift < 64);
        if shift == 0 {
            return self;
        }
        match self.storage {
            IntStorage::Small(s) => {
                let high_bits = extract_high_n_bits(s, shift);
                let res = s.shl(shift);
                let bit_width = std::cmp::max(self.bit_width, width(res));
                if high_bits != 0 {
                    Int::big(vec![res, high_bits], bit_width + width(high_bits))
                } else {
                    Int::small(res, bit_width)
                }
            }
            IntStorage::Big(mut parts) => {
                todo!()
            }
        }
    }
}

impl<T> From<T> for Int
where
    T: Into<u64> + Sized,
{
    fn from(v: T) -> Self {
        Int::small(v.into(), (std::mem::size_of::<T>() * 8) as u64)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum IntStorage {
    Small(u64),
    Big(Vec<u64>),
}

fn width(v: u64) -> u64 {
    64 - (v.leading_zeros() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_add() {
        assert_eq!(Int::from(1u32) + Int::from(1u64), Int::from(2u64));
        assert_eq!(Int::from(0u32) + Int::from(1u32), Int::from(1u32));
        assert_eq!(Int::from(40u8) + Int::from(2u32), Int::from(42u32));
        assert_eq!(
            Int::from(u8::MAX) + Int::from(u8::MAX),
            Int::small((u8::MAX as u64) + (u8::MAX as u64), 9)
        );
        assert_eq!(
            Int::small(u8::MAX as u64, 10) + Int::from(u8::MAX),
            Int::small((u8::MAX as u64) + (u8::MAX as u64), 10)
        );
    }

    #[test]
    fn test_small_add_overflow_into_big() {
        assert_eq!(
            Int::from(u64::MAX) + Int::from(1u64),
            Int::big(vec![0u64, 1u64], 65)
        );
        assert_eq!(
            Int::from(u64::MAX) + Int::from(u64::MAX),
            Int::big(vec![u64::MAX - 1, 1u64], 65)
        );
    }

    #[test]
    fn test_small_big_add() {
        assert_eq!(
            Int::big(vec![0u64, 1u64], 65) + Int::from(1u64),
            Int::big(vec![1u64, 1u64], 65)
        );
        assert_eq!(
            Int::from(1u64) + Int::big(vec![u64::MAX, 1u64], 65),
            Int::big(vec![0u64, 2u64], 66)
        );
        assert_eq!(
            Int::big(vec![u64::MAX, u64::MAX, 1u64], 129) + Int::from(42u64),
            Int::big(vec![41u64, 0u64, 2u64], 130)
        );
        assert_eq!(
            Int::big(vec![u64::MAX, u64::MAX], 128) + Int::from(1u64),
            Int::big(vec![0u64, 0u64, 1u64], 129)
        );
    }

    #[test]
    fn test_big_big_add() {
        assert_eq!(
            Int::big(vec![0u64, 1u64], 65) + Int::big(vec![0u64, 1u64], 96),
            Int::big(vec![0u64, 2u64], 96)
        );
        assert_eq!(
            Int::big(vec![u64::MAX, 1u64], 65) + Int::big(vec![1u64, 0u64], 65),
            Int::big(vec![0u64, 2u64], 66)
        );
        assert_eq!(
            Int::big(vec![u64::MAX, 1], 65) + Int::big(vec![1, 2], 66),
            Int::big(vec![0, 4], 67)
        );
        assert_eq!(
            Int::big(vec![u64::MAX, u64::MAX], 128) + Int::big(vec![1, 0], 96),
            Int::big(vec![0, 0, 1], 129)
        );
        assert_eq!(
            Int::big(vec![u64::MAX, u64::MAX], 128) + Int::big(vec![0, 0], 128),
            Int::big(vec![u64::MAX, u64::MAX], 128)
        );
        assert_eq!(
            Int::big(vec![u64::MAX, u64::MAX], 128)
                + Int::big(vec![u64::MAX, u64::MAX], 128),
            Int::big(vec![u64::MAX - 1, u64::MAX, 1], 129)
        );
        assert_eq!(
            Int::big(vec![0, 0, 1, 1], 256) + Int::big(vec![1, 1], 65),
            Int::big(vec![1, 1, 1, 1], 256)
        );
    }

    #[test]
    fn test_shr() {
        assert_eq!(Int::small(64, 32) >> 2, Int::small(16, 32));
        assert_eq!(Int::big(vec![2, 2], 128) >> 1, Int::big(vec![1, 1], 128));
        assert_eq!(
            Int::big(vec![0, 1], 128) >> 1,
            Int::big(vec![1 << 63, 0], 128),
        );
        assert_eq!(
            Int::big(vec![0, 0b11111], 128) >> 4,
            Int::big(vec![0b1111 << 60, 0b1], 128),
        );
        assert_eq!(
            Int::big(vec![0, 0, 1], 196) >> 64,
            Int::big(vec![0, 1, 0], 196),
        );
        assert_eq!(
            Int::big(vec![0, 0, 1], 196) >> 66,
            Int::big(vec![1 << 62, 0, 0], 196),
        );
    }

    #[test]
    fn test_shl() {
        assert_eq!(Int::small(2, 32) << 2, Int::small(8, 32));
        assert_eq!(Int::small(1 << 31, 32) << 2, Int::small(1 << 33, 34));
        assert_eq!(Int::small(1 << 63, 64) << 1, Int::big(vec![0, 1], 65));
        assert_eq!(
            Int::small((0b11) << 62 | 0b11, 64) << 2,
            Int::big(vec![0b1100, 0b11], 66)
        );
        //assert_eq!(Int::big(vec![1, 1], 128) << 1, Int::big(vec![2, 2], 128));
        //assert_eq!(
        //    Int::big(vec![1 << 63, 0], 128) >> 1,
        //    Int::big(vec![0, 1], 128),
        //);
        //assert_eq!(
        //    Int::big(vec![0b1111 << 60, 0b1], 128) << 4,
        //    Int::big(vec![0, 0b11111], 128),
        //);
        //assert_eq!(
        //    Int::big(vec![0, 0, 1], 196) >> 64,
        //    Int::big(vec![0, 1, 0], 196),
        //);
        //assert_eq!(
        //    Int::big(vec![0, 0, 1], 196) >> 66,
        //    Int::big(vec![1 << 62, 0, 0], 196),
        //);
    }
}
