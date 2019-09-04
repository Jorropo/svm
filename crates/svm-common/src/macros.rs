/// `impl_bytes_primitive` macro implements a struct consisting of one
#[macro_export]
macro_rules! impl_bytes_primitive {
    ($primitive: ident, $bytes_count: expr) => {
        /// Spacemesh `$primitive` consists of `$bytes_count` bytes.
        #[derive(serde::Serialize, serde::Deserialize, Debug, Copy, Clone, Hash, PartialEq, Eq)]
        #[repr(transparent)]
        pub struct $primitive(pub(self) [u8; $bytes_count]);

        impl From<&[u8]> for $primitive {
            fn from(slice: &[u8]) -> $primitive {
                assert_eq!($bytes_count, slice.len());

                let mut buf: [u8; $bytes_count] = [0; $bytes_count];
                buf.copy_from_slice(slice);

                $primitive(buf)
            }
        }

        impl From<*const u8> for $primitive {
            fn from(ptr: *const u8) -> $primitive {
                let slice: &[u8] = unsafe { std::slice::from_raw_parts(ptr, $bytes_count) };

                $primitive::from(slice)
            }
        }

        impl $primitive {
            /// Returns a raw pointer into the `$primitive` internal array
            pub fn as_ptr(&self) -> *const u8 {
                self.0.as_ptr()
            }

            /// Returns a slice into the `$primitive` internal array
            pub fn as_slice(&self) -> &[u8] {
                &self.0[..]
            }

            /// Returns a clone of the `$primitive` internal array
            pub fn bytes(&self) -> [u8; $bytes_count] {
                self.0
            }

            /// Returns the number of bytes of `$primitive`
            #[inline(always)]
            pub fn len() -> usize {
                return $bytes_count;
            }
        }

        /// Should be used **only** for tests
        #[doc(hidden)]
        impl From<u32> for $primitive {
            fn from(n: u32) -> $primitive {
                let mut buf = [0; $bytes_count];

                let [n0, n1, n2, n3] = $crate::utils::u32_to_le_array(n);

                buf[0] = n0;
                buf[1] = n1;
                buf[2] = n2;
                buf[3] = n3;

                $primitive(buf)
            }
        }

        /// Should be used **only** for tests
        #[doc(hidden)]
        impl From<i32> for $primitive {
            #[inline(always)]
            fn from(n: i32) -> $primitive {
                $primitive::from(n as u32)
            }
        }

        /// Should be used **only** for tests
        #[doc(hidden)]
        impl From<u64> for $primitive {
            fn from(n: u64) -> $primitive {
                let mut buf = [0; $bytes_count];

                let [n0, n1, n2, n3, n4, n5, n6, n7] = $crate::utils::u64_to_le_array(n);

                buf[0] = n0;
                buf[1] = n1;
                buf[2] = n2;
                buf[3] = n3;
                buf[4] = n4;
                buf[5] = n5;
                buf[6] = n6;
                buf[7] = n7;

                $primitive(buf)
            }
        }
    };
}
