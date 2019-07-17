use crate::page::{PageHash, PageIndex};
use crate::traits::PageHasher;
use svm_common::{Address, DefaultKeyHasher, KeyHasher};

use std::marker::PhantomData;
use std::ops::Add;

pub struct PageHasherImpl<H> {
    marker: PhantomData<H>,
}

impl<H: KeyHasher<Hash = [u8; 32]>> PageHasher for PageHasherImpl<H> {
    /// page_addr = contract_addr + page_idx
    /// ph = HASH(page_addr || HASH(page_data))
    fn hash(contract_addr: Address, page_idx: PageIndex, page_data: &[u8]) -> PageHash {
        let page_data_hash = H::hash(&page_data);
        let page_addr: [u8; 33] = contract_addr.add(page_idx.0);

        let mut data = Vec::with_capacity(page_data_hash.len() + page_addr.len());

        data.extend_from_slice(&page_addr);
        data.extend_from_slice(&page_data_hash);

        let ph = H::hash(&data);

        PageHash(ph)
    }
}

/// A default implementation for `PageHasher` trait.
pub type DefaultPageHasher = PageHasherImpl<DefaultKeyHasher>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_page_hasher_sanity() {
        let page_addr = [
            0x14, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let page_data_hash = DefaultKeyHasher::hash(&[10, 20, 30]);
        let mut data = Vec::with_capacity(page_addr.len() + page_data_hash.len());
        data.extend_from_slice(&page_addr);
        data.extend_from_slice(&page_data_hash);
        let expected = DefaultKeyHasher::hash(data.as_slice());

        let addr = Address::from(0x44_33_22_11 as u32);
        let page_idx = PageIndex(3);
        let page_data = vec![10, 20, 30];

        let actual = DefaultPageHasher::hash(addr, page_idx, page_data.as_slice());

        assert_eq!(expected, actual.0);
    }
}
