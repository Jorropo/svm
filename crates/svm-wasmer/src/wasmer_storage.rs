use memoffset::{offset_of, span_of};
use wasmer_runtime_core::vm::Ctx;

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;
use svm_common::Address;

use svm_storage::null_storage::{NullPageCache, NullPageSliceCache, NullPagesStorage};
use svm_storage::traits::PagesStorage;
use svm_storage::{MemKVStore, MemPages, PageCache, PageSliceCache};

use super::wasmer_register::WasmerReg64;

/// The number of allocated `64-bit` wasmer registers for each `SvmCtx`
pub const REGS_64_COUNT: usize = 8;

/// `SvmCtx` is a container for the accessible data by `wasmer` instances
/// Its fields are:
/// * `regs_64` - a static array (`REGS_64_COUNT` elements) of `WasmerReg64`
///
/// Explanation about `SvmCtx` lifetimes and generics:
/// * `a  - the lifetime of the mutable borrowed `PageSliceCache`
/// * `pc - the lifetime of the inner `PageCache` within `PageSliceCache` (`pc - stands for page-cache)
/// *  PS - a type implementing the trait `PagesStorage` (`PS` stands for `PagesStorage`)
#[repr(C)]
pub struct SvmCtx<'a, 'pc: 'a, PS> {
    pub(crate) regs_64: [WasmerReg64; REGS_64_COUNT],

    pub(crate) storage: &'a mut PageSliceCache<'pc, PS>,
}

impl<'a, 'pc: 'a, PS> SvmCtx<'a, 'pc, PS> {
    /// Initializes a new empty `SvmCtx`
    ///
    /// params:
    /// * `storage` - a mutably borrowed `PageSliceCache`
    pub fn new(storage: &'a mut PageSliceCache<'pc, PS>) -> Self {
        let regs_64 = [WasmerReg64::new(); REGS_64_COUNT];

        Self { regs_64, storage }
    }
}

macro_rules! ctx_regs_reg {
    ($regs: expr, $reg_idx: expr) => {{
        assert!($reg_idx >= 0 && $reg_idx < REGS_64_COUNT as i32);

        /// We don't do:
        /// ```rust
        /// let reg: &mut WasmerReg64 = $regs.regs_64[$reg_idx as usize];
        /// ```
        ///
        /// Because we like to keep the option to  mutate a couple of registers simultaneously
        /// without the Rust borrow checker getting angry...
        /// so instead we use _Unsafe Rust_
        let regs_ptr: *mut WasmerReg64 = $regs.as_mut_ptr();

        let reg_idx_ptr: *mut WasmerReg64 = unsafe { regs_ptr.offset($reg_idx as isize) };

        let reg: &mut WasmerReg64 = unsafe { &mut *reg_idx_ptr };

        reg
    }};
}

/// Casts the `wasmer` instance context (type: `Ctx`) data field (of type `*mut c_void`) into `&mut [WasmerReg64; REGS_64_COUNT]`
macro_rules! wasmer_ctx_data_regs {
    ($data: expr) => {{
        let data_ptr = $data as *mut _;

        let regs: &mut [WasmerReg64; REGS_64_COUNT] = unsafe { &mut *data_ptr };

        regs
    }};
}

/// Casts the `wasmer` instance context (type: `Ctx`) data field (of type `*mut c_void`) into `&mut PageSliceCache<PS>`
macro_rules! wasmer_ctx_data_storage {
    ($data: expr, $PS: ident) => {{
        let data_ptr: *mut u8 = $data as *mut _;

        // TODO: figure out why we need to add `std::mem::usize_of::<usize>()` to the starting offset of `SvmCtx` storage
        let storage_offset =
            std::mem::size_of::<WasmerReg64>() * REGS_64_COUNT + std::mem::size_of::<usize>();

        let storage_ptr: *mut u8 = unsafe { data_ptr.offset(storage_offset as isize) };

        let storage_ptr = storage_ptr as *mut PageSliceCache<$PS>;

        let storage: &mut PageSliceCache<$PS> = unsafe { &mut *storage_ptr };

        storage
    }};
}

/// Returns a `wasmer` memory view of cells `mem_start, mem_start + 1, .. , mem_start + len` (exclusive)
macro_rules! wasmer_mem_cells {
    ($ctx: expr, $mem_start: expr, $len: expr) => {{
        let start = $mem_start as usize;
        let end = start + $len as usize;

        &$ctx.memory(0).view()[start..end]
    }};
}

/// Extracts from `wasmer` instance context (type: `Ctx`) a mutable borrow for the register indexed `reg_idx`
macro_rules! wasmer_reg {
    ($ctx: expr, $reg_idx: expr) => {{
        assert!($reg_idx >= 0 && $reg_idx < REGS_64_COUNT as i32);

        let regs = wasmer_ctx_data_regs!($ctx.data);

        ctx_regs_reg!(regs, $reg_idx)
    }};
}

/// Copies the content of `wasmer` memory cells under addresses:
/// `src_mem_ptr, src_mem_ptr + 1, .. , src_mem_ptr + len (exclusive)`
/// into `wasmer` register indexed `dst_reg`
pub fn mem_to_reg_copy(ctx: &mut Ctx, src_mem_ptr: i32, dst_reg: i32, len: i32) {
    let reg = wasmer_reg!(ctx, dst_reg);
    let cells = wasmer_mem_cells!(ctx, src_mem_ptr, len);

    reg.copy_from_wasmer_mem(cells);
}

/// Copies the content of `wasmer` register indexed `src_reg` into `wasmer` memory cells under addresses:
/// `dst_mem_ptr, dst_mem_ptr + 1, .. , dst_mem_ptr + len (exclusive)`
pub fn reg_to_mem_copy<PS: PagesStorage>(ctx: &mut Ctx, src_reg: i32, dst_mem_ptr: i32, len: i32) {
    let reg = wasmer_reg!(ctx, src_reg);
    let cells = wasmer_mem_cells!(ctx, dst_mem_ptr, len);

    reg.copy_to_wasmer_mem(cells);
}

pub fn storage_read_to_reg(ctx: &mut Ctx, _page: i32, _offset: i32, _len: i32, _reg: i32) {
    //
}

// // #[allow(unused)]
// // pub fn storage_set_from_reg(_ctx: &mut Ctx, _page: i32, _offset: i32, _reg: i32) {
// //     //
// // }

#[cfg(test)]
mod tests {
    use super::*;

    use std::cell::{Cell, RefCell};
    use std::ffi::c_void;
    use std::rc::Rc;

    use svm_storage::{PageIndex, PageSliceLayout, SliceIndex};

    pub type MemPageCache<'pc, K = [u8; 32]> = PageCache<'pc, MemPages<K>>;

    fn wasmer_import_object_data<PS: PagesStorage>(
        ctx: &SvmCtx<PS>,
    ) -> (*mut c_void, fn(*mut c_void)) {
        let data: *mut c_void = ctx.clone() as *const _ as *mut c_void;
        let dtor: fn(*mut c_void) = |_| {};

        (data, dtor)
    }

    #[test]
    fn reg_copy_from_wasmer_mem() {
        let mut pages = NullPagesStorage::new();
        let mut page_cache = NullPageCache::new(&mut pages, 1);
        let mut storage = NullPageSliceCache::new(&mut page_cache, 10);
        let ctx = SvmCtx::new(&mut storage);

        let (data, _dtor) = wasmer_import_object_data(&ctx);

        let regs = wasmer_ctx_data_regs!(data);

        let reg0: &mut WasmerReg64 = ctx_regs_reg!(regs, 0);
        let reg1: &mut WasmerReg64 = ctx_regs_reg!(regs, 1);

        // registers `0` and `1` are initialized with zeros
        assert_eq!(vec![0; 8], &reg0.0[..]);
        assert_eq!(vec![0; 8], &reg1.0[..]);

        // editing register `0` should not edit register `1`
        let cells = vec![
            Cell::new(10),
            Cell::new(20),
            Cell::new(30),
            Cell::new(40),
            Cell::new(50),
            Cell::new(60),
            Cell::new(70),
            Cell::new(80),
        ];

        reg0.copy_from_wasmer_mem(&cells);

        assert_eq!(vec![10, 20, 30, 40, 50, 60, 70, 80], &reg0.0[..]);
        assert_eq!(vec![00, 00, 00, 00, 00, 00, 00, 00], &reg1.0[..]);
    }

    #[test]
    fn reg_copy_to_wasmer_mem() {
        let mut pages = NullPagesStorage::new();
        let mut page_cache = NullPageCache::new(&mut pages, 1);
        let mut storage = NullPageSliceCache::new(&mut page_cache, 10);
        let ctx = SvmCtx::new(&mut storage);

        let (data, _dtor) = wasmer_import_object_data(&ctx);

        let regs = wasmer_ctx_data_regs!(data);
        let reg0: &mut WasmerReg64 = ctx_regs_reg!(regs, 0);

        // initialize register `0` with data
        let cells = vec![
            Cell::new(10),
            Cell::new(20),
            Cell::new(30),
            Cell::new(40),
            Cell::new(50),
            Cell::new(60),
            Cell::new(70),
            Cell::new(80),
        ];

        reg0.copy_from_wasmer_mem(&cells);
        assert_eq!(vec![10, 20, 30, 40, 50, 60, 70, 80], &reg0.0[..]);

        // copying register `0` to a fake wasmer memory at cells: `10, 11, ... 17` (inclusive)
        let cells: Vec<Cell<u8>> = (0..8).map(|_| Cell::<u8>::new(0)).collect();

        assert_eq!(
            vec![00, 00, 00, 00, 00, 00, 00, 00],
            cells.iter().map(|c| c.get()).collect::<Vec<u8>>()
        );

        // copying register `0` content into memory
        reg0.copy_to_wasmer_mem(&cells);

        // asserting that the fake wasmer memory has the right changes
        assert_eq!(
            vec![10, 20, 30, 40, 50, 60, 70, 80],
            cells.iter().map(|c| c.get()).collect::<Vec<u8>>()
        );
    }

    #[test]
    fn wasmer_storage_read_write() {
        let addr = Address::from(0x12_34_56_78 as u32);
        let db = Rc::new(RefCell::new(MemKVStore::new()));
        let mut inner = MemPages::new(addr, db);
        let mut page_cache = MemPageCache::new(&mut inner, 1);
        let mut storage = PageSliceCache::new(&mut page_cache, 10);
        let ctx = SvmCtx::new(&mut storage);

        let (data, _dtor) = wasmer_import_object_data(&ctx);

        // extracting `storage` out of `wasmer` instance `data` field
        let storage: &mut PageSliceCache<NullPageCache> =
            wasmer_ctx_data_storage!(data, NullPageCache);

        let layout = PageSliceLayout {
            slice_idx: SliceIndex(0),
            page_idx: PageIndex(1),
            offset: 100,
            len: 3,
        };

        assert_eq!(Vec::<u8>::new(), storage.read_page_slice(&layout).unwrap());

        storage.write_page_slice(&layout, vec![10, 20, 30]);

        // starting over, extracting `storage` out of `wasmer` instance `data` field
        let storage = wasmer_ctx_data_storage!(data, NullPageCache);
        assert_eq!(vec![10, 20, 30], storage.read_page_slice(&layout).unwrap());
    }
}
