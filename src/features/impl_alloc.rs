use crate::{
    de::{decode_slice_len, Decode, Decoder},
    enc::{self, Encode, Encoder},
    error::{DecodeError, EncodeError},
    Config,
};
#[cfg(feature = "atomic")]
use alloc::sync::Arc;
use alloc::{
    borrow::{Cow, ToOwned},
    boxed::Box,
    rc::Rc,
    string::String,
    vec::Vec,
};

#[derive(Default)]
pub(crate) struct VecWriter {
    inner: Vec<u8>,
}

impl VecWriter {
    // May not be used in all feature combinations
    #[allow(dead_code)]
    pub(crate) fn collect(self) -> Vec<u8> {
        self.inner
    }
}

impl enc::write::Writer for VecWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), EncodeError> {
        self.inner
            .try_reserve(bytes.len())
            .map_err(|inner| EncodeError::OutOfMemory { inner })?;

        let start = self.inner.len();
        let target = unsafe {
            // Safety: We reserved above so we know we can set the length, and we're filling the value right below this
            self.inner.set_len(start + bytes.len());
            // Safety: We know we have this slice available
            self.inner.get_unchecked_mut(start..start + bytes.len())
        };
        target.copy_from_slice(bytes);
        Ok(())
    }
}

/// Encode the given value into a `Vec<u8>` with the given `Config`. See the [config] module for more information.
///
/// [config]: config/index.html
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub fn encode_to_vec<E: enc::Encode, C: Config>(val: E, config: C) -> Result<Vec<u8>, EncodeError> {
    let writer = VecWriter::default();
    let mut encoder = enc::EncoderImpl::<_, C>::new(writer, config);
    val.encode(&mut encoder)?;
    Ok(encoder.into_writer().inner)
}

#[cfg(not(no_global_oom_handling))]
mod collection_impls {
    use super::*;
    use alloc::collections::*;

    impl<T> Decode for BinaryHeap<T>
    where
        T: Decode + Ord,
    {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<T>(len)?;

            let mut map = BinaryHeap::new();
            // TODO:
            // map.try_reserve(len).map_err(|inner| DecodeError::OutOfMemory { inner })?;
            map.reserve(len);

            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());

                let key = T::decode(decoder)?;
                map.push(key);
            }
            Ok(map)
        }
    }

    impl<T> Encode for BinaryHeap<T>
    where
        T: Encode + Ord,
    {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            crate::enc::encode_slice_len(encoder, self.len())?;
            for val in self.iter() {
                val.encode(encoder)?;
            }
            Ok(())
        }
    }

    impl<K, V> Decode for BTreeMap<K, V>
    where
        K: Decode + Ord,
        V: Decode,
    {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<(K, V)>(len)?;

            let mut map = BTreeMap::new();
            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<(K, V)>());

                let key = K::decode(decoder)?;
                let value = V::decode(decoder)?;
                map.insert(key, value);
            }
            Ok(map)
        }
    }

    impl<K, V> Encode for BTreeMap<K, V>
    where
        K: Encode + Ord,
        V: Encode,
    {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            crate::enc::encode_slice_len(encoder, self.len())?;
            for (key, val) in self.iter() {
                key.encode(encoder)?;
                val.encode(encoder)?;
            }
            Ok(())
        }
    }

    impl<T> Decode for BTreeSet<T>
    where
        T: Decode + Ord,
    {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<T>(len)?;

            let mut map = BTreeSet::new();
            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());

                let key = T::decode(decoder)?;
                map.insert(key);
            }
            Ok(map)
        }
    }

    impl<T> Encode for BTreeSet<T>
    where
        T: Encode + Ord,
    {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            crate::enc::encode_slice_len(encoder, self.len())?;
            for item in self.iter() {
                item.encode(encoder)?;
            }
            Ok(())
        }
    }

    impl<T> Decode for VecDeque<T>
    where
        T: Decode,
    {
        fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
            let len = crate::de::decode_slice_len(decoder)?;
            decoder.claim_container_read::<T>(len)?;

            let mut map = VecDeque::new();
            map.try_reserve(len).map_err(|inner| {
                DecodeError::OutOfMemory(crate::error::OutOfMemory::TryReserve(inner))
            })?;

            for _ in 0..len {
                // See the documentation on `unclaim_bytes_read` as to why we're doing this here
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());

                let key = T::decode(decoder)?;
                map.push_back(key);
            }
            Ok(map)
        }
    }

    impl<T> Encode for VecDeque<T>
    where
        T: Encode,
    {
        fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
            crate::enc::encode_slice_len(encoder, self.len())?;
            for item in self.iter() {
                item.encode(encoder)?;
            }
            Ok(())
        }
    }
}

impl<T> Decode for Vec<T>
where
    T: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        use core::mem::MaybeUninit;

        let len = crate::de::decode_slice_len(decoder)?;
        decoder.claim_container_read::<T>(len)?;

        let mut vec = Vec::new();
        vec.try_reserve(len).map_err(|inner| {
            DecodeError::OutOfMemory(crate::error::OutOfMemory::TryReserve(inner))
        })?;

        let slice = vec.spare_capacity_mut();

        struct Guard<'a, T> {
            slice: &'a mut [MaybeUninit<T>],
            idx: usize,
        }

        impl<'a, T> Drop for Guard<'a, T> {
            fn drop(&mut self) {
                unsafe {
                    for item in &mut self.slice[..self.idx] {
                        core::ptr::drop_in_place(item as *mut MaybeUninit<T> as *mut T);
                    }
                }
            }
        }

        let mut guard = Guard { slice, idx: 0 };

        for _ in 0..len {
            // See the documentation on `unclaim_bytes_read` as to why we're doing this here
            decoder.unclaim_bytes_read(core::mem::size_of::<T>());

            let t = T::decode(decoder)?;
            guard.slice[guard.idx].write(t);
            guard.idx += 1;
        }
        // Don't drop the guard
        core::mem::forget(guard);
        unsafe {
            // All values are written, we can now set the length of the vec
            vec.set_len(vec.len() + len)
        }
        Ok(vec)
    }
}

impl<T> Encode for Vec<T>
where
    T: Encode,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        crate::enc::encode_slice_len(encoder, self.len())?;
        for item in self.iter() {
            item.encode(encoder)?;
        }
        Ok(())
    }
}

impl Decode for String {
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let bytes = Vec::<u8>::decode(decoder)?;
        String::from_utf8(bytes).map_err(|e| DecodeError::Utf8(e.utf8_error()))
    }
}

impl Encode for String {
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.as_bytes().encode(encoder)
    }
}

impl<T> Decode for Box<T>
where
    T: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::decode(decoder)?;
        Box::try_new(t).map_err(|e| DecodeError::OutOfMemory(crate::error::OutOfMemory::Alloc(e)))
    }
}

impl<T> Encode for Box<T>
where
    T: Encode,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        T::encode(self, encoder)
    }
}

impl<T> Decode for Box<[T]>
where
    T: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let len = decode_slice_len(decoder)?;
        decoder.claim_container_read::<T>(len)?;

        unsafe {
            use core::mem::MaybeUninit;
            let mut result = Box::try_new_uninit_slice(len)
                .map_err(|e| DecodeError::OutOfMemory(crate::error::OutOfMemory::Alloc(e)))?;

            struct Guard<'a, T> {
                result: &'a mut Box<[MaybeUninit<T>]>,
                initialized: usize,
                max: usize,
            }

            impl<T> Drop for Guard<'_, T> {
                fn drop(&mut self) {
                    debug_assert!(self.initialized <= self.max);

                    // SAFETY: this slice will contain only initialized objects.
                    unsafe {
                        let slice = &mut *(self.result.get_unchecked_mut(..self.initialized)
                            as *mut [MaybeUninit<T>]
                            as *mut [T]);
                        core::ptr::drop_in_place(slice);
                    }
                }
            }

            let mut guard = Guard {
                result: &mut result,
                initialized: 0,
                max: len,
            };

            while guard.initialized < guard.max {
                decoder.unclaim_bytes_read(core::mem::size_of::<T>());
                let t = T::decode(decoder)?;

                guard.result.get_unchecked_mut(guard.initialized).write(t);
                guard.initialized += 1;
            }

            core::mem::forget(guard);
            let (raw, alloc) = Box::into_raw_with_allocator(result);
            Ok(Box::from_raw_in(raw as *mut [T], alloc))
        }
    }
}

impl<T> Encode for Box<[T]>
where
    T: Encode,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        crate::enc::encode_slice_len(encoder, self.len())?;
        for item in self.iter() {
            item.encode(encoder)?;
        }
        Ok(())
    }
}

// BlockedTODO: https://github.com/rust-lang/rust/issues/31844
// Cow should be able to decode a borrowed value
// Currently this conflicts with the owned `Decode` implementation below

// impl<'cow, T> BorrowDecode<'cow> for Cow<'cow, T>
// where
//     T: BorrowDecode<'cow>,
// {
//     fn borrow_decode<D: crate::de::BorrowDecoder<'cow>>(decoder: &mut D) -> Result<Self, DecodeError> {
//         let t = T::borrow_decode(decoder)?;
//         Ok(Cow::Borrowed(t))
//     }
// }

impl<'cow, T> Decode for Cow<'cow, T>
where
    T: ToOwned,
    <T as ToOwned>::Owned: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = <T as ToOwned>::Owned::decode(decoder)?;
        Ok(Cow::Owned(t))
    }
}

impl<'cow, T> Encode for Cow<'cow, T>
where
    T: Encode + Clone,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        self.as_ref().encode(encoder)
    }
}

impl<T> Decode for Rc<T>
where
    T: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::decode(decoder)?;
        Rc::try_new(t).map_err(|e| DecodeError::OutOfMemory(crate::error::OutOfMemory::Alloc(e)))
    }
}

impl<T> Encode for Rc<T>
where
    T: Encode,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        T::encode(self, encoder)
    }
}

#[cfg(feature = "atomic")]
impl<T> Decode for Arc<T>
where
    T: Decode,
{
    fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
        let t = T::decode(decoder)?;
        Arc::try_new(t).map_err(|e| DecodeError::OutOfMemory(crate::error::OutOfMemory::Alloc(e)))
    }
}

#[cfg(feature = "atomic")]
impl<T> Encode for Arc<T>
where
    T: Encode,
{
    fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
        T::encode(self, encoder)
    }
}
