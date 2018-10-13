//! A low-level, zero-copy, panic-free, binary serializer and deserializer
//!
//! # Usage
//!
//! First, add the following to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! byte = "0.2"
//! ```
//!
//! Next, add this to your crate root:
//!
//! ```
//! extern crate byte;
//! ```
//!
//! `Byte` is `no_std` library; it can directly be used in a `#![no_std]` situation or crate.
//!
//!
//! # Overview
//!
//! `Byte` is mainly used to encode and decode binary data with standard or protocol,
//! such as network TCP packages and hardware communication packages.
//! It's similar to crate `nom` but more ligthweight and specialized for operating binary in low-level and hardware programing.
//!
//! `Byte` delivers two core traits `TryRead` and `TryWrite`.
//! Types implement these traits can be serialize into or deserialize from byte slices.
//! Byte slices `[u8]` derives methods `read()` and `write()` to serialize, deserialize and handle offset.
//!
//! Small and general is kept in mind in this library.
//! For example, `Byte` can take byte slice from [**MMap**](https://crates.io/crates/mmap) to read binary file,
//! or take heap-allocated byte buffer from [**Bytes**](https://github.com/carllerche/bytes).
//!
//!
//! # Example
//!
//! `Byte` consumes byte slice continuously. The first parameter of `read` is offset,
//! instructing the position to begin,
//! which will be increaed by size the operation consumed.
//! Reading a types usually requires some context, such as the endian of number,
//! in such situations, `read_with` is used and we can pass the context as the second parameter.
//!
//! ```
//! use byte::*;
//!
//! let bytes: &[u8] = &[0xde, 0xad, 0xbe, 0xef];
//!
//! let offset = &mut 0;
//! let num = bytes.read_with::<u32>(offset, BE).unwrap();
//! assert_eq!(num, 0xdeadbeef);
//! assert_eq!(*offset, 4);
//! ```
//!
//! ```
//! use byte::*;
//! use byte::ctx::{Str, NULL};
//!
//! let bytes: &[u8] = b"hello, world!\0dump";
//!
//! let offset = &mut 0;
//! let str = bytes.read_with::<&str>(offset, Str::Delimiter(NULL)).unwrap();
//! assert_eq!(str, "hello, world!");
//! assert_eq!(*offset, 14);
//! ```
//!
//! `Byte` supports language primitives by default.
//!
//! - `&str` (with context `Str`)
//! - `&[u8]` (with context `Byte`)
//! - `u8`, `i8`, `u64`, `f64` ... (with context `Endian`)
//! - `bool`
//!
//! `&str` and `&[u8]` are references to the byte slice so there is no copy when `read()` and it has the same lifetime as the byte slice.
//!
//! # Define custom serializable type
//!
//! In this example, we defined a custom type `Header`, which have a varibal-length name and a `bool` field.
//! We implement `TryRead` and `TryWrite` to enable this type to be serialzed and deserialized.
//!
//! Below is an example of communication protocol
//!
//! ## Protocol Byte Representation
//!
//! ```text
//! |       | Length of name (Big Endian) |                Name              | Enabled |
//! | ----- | --------------------------- | ---- | ---- | ---- | ---- | ---- | ------- |
//! | Byte  | 0            | 5            | 'H'  | 'E'  | 'L'  | 'L'  | 'O'  | 0       |
//! ```
//!
//! Below is the code to realize a reader and writer to it.
//!
//! Note that the `bytes` passed in is splitted by offset and should be read at head.
//! Type `Result` is an alias as `core::result::Result<(T, size), byte::Error>`,
//! where the size is the number of bytes `read` or `write` consumed and it will be used to increase the offset.
//!
//! ```
//! use byte::*;
//! use byte::ctx::*;
//!
//! struct Header<'a> {
//!     name: &'a str,
//!     enabled: bool,
//! }
//!
//! impl<'a> TryRead<'a, Endian> for Header<'a> {
//!     fn try_read(bytes: &'a [u8], endian: Endian) -> Result<(Self, usize)> {
//!         let offset = &mut 0;
//!
//!         let name_len = bytes.read_with::<u16>(offset, endian)? as usize;
//!         let header = Header {
//!             name: bytes.read_with::<&str>(offset, Str::Len(name_len))?,
//!             enabled: bytes.read::<bool>(offset)?,
//!         };
//!
//!         Ok((header, *offset))
//!     }
//! }
//!
//! impl<'a> TryWrite<Endian> for Header<'a> {
//!     fn try_write(self, bytes: &mut [u8], endian: Endian) -> Result<usize> {
//!         let offset = &mut 0;
//!
//!         bytes.write_with::<u16>(offset, self.name.len() as u16, endian)?;
//!         bytes.write::<&str>(offset, self.name)?;
//!         bytes.write::<bool>(offset, self.enabled)?;
//!
//!         Ok(*offset)
//!     }
//! }
//! ```
//!
//! ## Usage
//!
//! ```ignore
//! let bytes = [0, 5, b"H"[0], b"E"[0], b"L"[0], b"L"[0], b"O"[0], 0];
//!
//! let header: Header = bytes.read_with(&mut 0, BE).unwrap();
//! assert_eq!(header.name, "HELLO");
//! assert_eq!(header.enabled, false);
//!
//! let mut write = [0u8; 8];
//! write.write_with(&mut 0, header, BE).unwrap();
//! assert_eq!(write, bytes);
//! ```

#![no_std]

pub mod ctx;
use core::marker::PhantomData;
pub use ctx::{BE, LE};

/// A specialized Result type for `Byte`
pub type Result<T> = core::result::Result<T, Error>;

/// The error type for serializing and deserializing.
///
/// - `Error::BadOffset` should only raised in `bytes.read()` and `bytes.write()`
/// when offset exceeded slice's length.
///
/// - `Error::BadInput` and `Error::Incomplete` should only raised in `try_read()` and `try_write()`.
///
/// Note that we usually use `bytes.read()` in `try_read()` implementation which may raises `Error::BadOffset`,
/// and that error will automatically be translate into `Error::Incomplete`.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Error {
    /// The requested data is bigger than available range
    Incomplete,
    /// The offset is invalid
    BadOffset(usize),
    /// The requested data content is invalid
    BadInput { err: &'static str },
}

/// A shorthand function to check whether the given length
/// exceeded the length of slice; return `Err(Error::Incomplete)` if not.
///
/// Usually used in implementation of `TryRead` and `TryWrite`.
///
/// # Example
///
/// ```
/// use byte::*;
///
/// let bytes = [0u8; 4];
/// assert_eq!(check_len(&bytes, 4), Ok(4));
/// assert_eq!(check_len(&bytes, 5), Err(Error::Incomplete));
/// ```
#[inline]
pub fn check_len(bytes: &[u8], len: usize) -> Result<usize> {
    if bytes.len() < len {
        Err(Error::Incomplete)
    } else {
        Ok(len)
    }
}

/// A data structure that can be deserialized. Types implement this trait can be `read()` from byte slice.
pub trait TryRead<'a, Ctx = ()>
where
    Self: Sized,
{
    /// Try to read from bytes using context.
    ///
    /// Read the value out of bytes; the bytes passed in is splitted by offset and should be read at head.
    /// If success, `try_read()` should return a tuple with the value and the number of bytes consumed.
    ///
    /// # Example
    ///
    /// ```
    /// use byte::*;
    ///
    /// // Demo type showing how to read boolean from bytes.
    /// // This functionality is already provided by this crate.
    /// pub struct Bool(bool);
    ///
    /// impl<'a> TryRead<'a> for Bool {
    ///     #[inline]
    ///     fn try_read(bytes: &'a [u8], _ctx: ()) -> Result<(Self, usize)> {
    ///         check_len(bytes, 1)?;
    ///
    ///         Ok((Bool(bytes[0] != 0), 1))
    ///     }
    /// }
    /// ```
    fn try_read(bytes: &'a [u8], ctx: Ctx) -> Result<(Self, usize)>;
}

/// A data structure that can be serialized. Types implement this trait can be `write()` into byte slice.
pub trait TryWrite<Ctx = ()> {
    /// Try to write to bytes using context.
    ///
    /// Write the value into bytes; the bytes passed in is splitted by offset and should be write at head.
    /// If success, `try_write()` should return the number of bytes written.
    ///
    /// # Example
    ///
    /// ```
    /// use byte::*;
    ///
    /// pub struct HasBool(bool);
    ///
    /// impl TryWrite for HasBool {
    ///     #[inline]
    ///     fn try_write(self, bytes: &mut [u8], _ctx: ()) -> Result<usize> {
    ///         check_len(bytes, 1)?;
    ///
    ///         bytes[0] = if self.0 { u8::max_value() } else { 0 };
    ///
    ///         Ok(1)
    ///     }
    /// }
    /// ```
    fn try_write(self, bytes: &mut [u8], ctx: Ctx) -> Result<usize>;
}

/// Extension methods for byte slices.
///
/// # Offset
///
/// The first parameter of each method is offset,
/// instructing the position to begin,
/// which will be increaed by size the operation consumed.
pub trait BytesExt<Ctx> {
    /// Read value from byte slice by default context
    ///
    /// # Example
    ///
    /// ```
    /// use byte::*;
    ///
    /// let bytes: &[u8] = &[0, 1];
    ///
    /// let bool1: bool = bytes.read(&mut 0).unwrap();
    /// let bool2: bool = bytes.read(&mut 1).unwrap();
    ///
    /// assert_eq!(bool1, false);
    /// assert_eq!(bool2, true);
    /// ```
    fn read<'a, T>(&'a self, offset: &mut usize) -> Result<T>
    where
        T: TryRead<'a, Ctx>,
        Ctx: Default,
    {
        self.read_with(offset, Default::default())
    }

    /// Read value from byte slice with context
    ///
    /// # Example
    ///
    /// ```
    /// use byte::*;
    /// use byte::ctx::*;
    ///
    /// let bytes: &[u8] = b"hello, world!";
    ///
    /// let str: &str = bytes.read_with(&mut 0, Str::Delimiter(b"!"[0])).unwrap();
    /// assert_eq!(str, "hello, world");
    /// ```
    fn read_with<'a, T>(&'a self, offset: &mut usize, ctx: Ctx) -> Result<T>
    where
        T: TryRead<'a, Ctx>;

    /// Read multiple values of same type by iterator.
    ///
    /// # Example
    ///
    /// ```
    /// use byte::*;
    /// use byte::ctx::*;
    ///
    /// let bytes: &[u8] = b"hello\0world\0dead\0beef\0more";
    /// let mut offset = 0;
    /// {
    ///     let mut iter = bytes.read_iter(&mut offset, Str::Delimiter(NULL));
    ///     assert_eq!(iter.next(), Some("hello"));
    ///     assert_eq!(iter.next(), Some("world"));
    ///     assert_eq!(iter.next(), Some("dead"));
    ///     assert_eq!(iter.next(), Some("beef"));
    ///     assert_eq!(iter.next(), None);
    /// }
    /// assert_eq!(offset, 22);
    /// ```
    fn read_iter<'a, 'i, T>(&'a self, offset: &'i mut usize, ctx: Ctx) -> Iter<'a, 'i, T, Ctx>
    where
        T: TryRead<'a, Ctx>,
        Ctx: Clone;

    /// Write value into byte slice by default context
    ///
    /// # Example
    ///
    /// ```
    /// use byte::*;
    ///
    /// let mut bytes = [0u8; 2];
    ///
    /// bytes.write(&mut 0, false).unwrap();
    /// bytes.write(&mut 1, true).unwrap();
    ///
    /// assert_eq!(bytes, [0, 0x01]);
    /// ```
    fn write<T>(&mut self, offset: &mut usize, t: T) -> Result<()>
    where
        T: TryWrite<Ctx>,
        Ctx: Default,
    {
        self.write_with(offset, t, Default::default())
    }

    /// Write value into byte slice with context
    ///
    /// # Example
    ///
    /// ```
    /// use byte::*;
    /// use byte::ctx::*;
    ///
    /// let mut bytes_be = [0u8; 2];
    /// let mut bytes_le = [0u8; 2];
    ///
    /// bytes_be.write_with::<u16>(&mut 0, 0xff, BE).unwrap();
    /// bytes_le.write_with::<u16>(&mut 0, 0xff, LE).unwrap();
    ///
    /// assert_eq!(bytes_be, [0, 0xff]);
    /// assert_eq!(bytes_le, [0xff, 0]);
    /// ```
    fn write_with<T>(&mut self, offset: &mut usize, t: T, ctx: Ctx) -> Result<()>
    where
        T: TryWrite<Ctx>;
}

impl<Ctx> BytesExt<Ctx> for [u8] {
    #[inline]
    fn read_with<'a, T>(&'a self, offset: &mut usize, ctx: Ctx) -> Result<T>
    where
        T: TryRead<'a, Ctx>,
    {
        let slice = self.as_ref();

        if *offset >= slice.len() {
            return Err(Error::BadOffset(*offset));
        };

        match TryRead::try_read(&slice[*offset..], ctx) {
            Ok((t, size)) => {
                *offset += size;
                Ok(t)
            }
            Err(Error::BadOffset(_)) => Err(Error::Incomplete),
            Err(err) => Err(err),
        }
    }

    fn read_iter<'a, 'i, T>(&'a self, offset: &'i mut usize, ctx: Ctx) -> Iter<'a, 'i, T, Ctx>
    where
        T: TryRead<'a, Ctx>,
        Ctx: Clone,
    {
        Iter {
            bytes: self.as_ref(),
            offset: offset,
            ctx: ctx,
            phantom: PhantomData,
        }
    }

    fn write_with<T>(&mut self, offset: &mut usize, t: T, ctx: Ctx) -> Result<()>
    where
        T: TryWrite<Ctx>,
    {
        let slice = self.as_mut();

        if *offset >= slice.len() {
            return Err(Error::BadOffset(*offset));
        };

        match TryWrite::try_write(t, &mut slice[*offset..], ctx) {
            Ok(size) => {
                *offset += size;
                Ok(())
            }
            Err(Error::BadOffset(_)) => Err(Error::Incomplete),
            Err(err) => Err(err),
        }
    }
}

/// Iterator that read values of same type from bytes.
///
/// # Example
///
/// ```
/// use byte::*;
/// use byte::ctx::*;
///
/// let bytes: &[u8] = b"hello\0world\0dead\0beef\0more";
/// let mut offset = 0;
/// {
///     let mut iter = bytes.read_iter(&mut offset, Str::Delimiter(NULL));
///     assert_eq!(iter.next(), Some("hello"));
///     assert_eq!(iter.next(), Some("world"));
///     assert_eq!(iter.next(), Some("dead"));
///     assert_eq!(iter.next(), Some("beef"));
///     assert_eq!(iter.next(), None);
/// }
/// assert_eq!(offset, 22);
/// ```
#[must_use = "iterator adaptors are lazy and do nothing unless consumed"]
pub struct Iter<'a, 'i, T, Ctx>
where
    T: TryRead<'a, Ctx>,
    Ctx: Clone,
{
    bytes: &'a [u8],
    offset: &'i mut usize,
    ctx: Ctx,
    phantom: PhantomData<T>,
}

impl<'a, 'i, T, Ctx> Iterator for Iter<'a, 'i, T, Ctx>
where
    T: TryRead<'a, Ctx>,
    Ctx: Clone,
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        TryRead::try_read(&self.bytes[*self.offset..], self.ctx.clone())
            .ok()
            .map(|(t, size)| {
                *self.offset += size;
                t
            })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
