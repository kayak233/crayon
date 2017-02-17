use std::marker::PhantomData;
use byteorder::ReadBytesExt;
use std::io::Read;
use std::u32;

use serde;
use serde::de::value::ValueDeserializer;
use serde::de::Error as DeError;
use byteorder;

use super::error::{Result, Error, ErrorKind};

/// A limit on the amount of bytes that can be read or written.
///
/// Size limits are an incredibly important part of both encoding and decoding.
///
/// In order to prevent DOS attacks on a decoder, it is important to limit the
/// amount of bytes that a single encoded message can be; otherwise, if you
/// are decoding bytes right off of a TCP stream for example, it would be
/// possible for an attacker to flood your server with a 3TB vec, causing the
/// decoder to run out of memory and crash your application!
/// Because of this, you can provide a maximum-number-of-bytes that can be read
/// during decoding, and the decoder will explicitly fail if it has to read
/// any more than that.
///
/// On the other side, you want to make sure that you aren't encoding a message
/// that is larger than your decoder expects.  By supplying a size limit to an
/// encoding function, the encoder will verify that the structure can be encoded
/// within that limit.  This verification occurs before any bytes are written to
/// the Writer, so recovering from an error is easy.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum SizeLimit {
    Infinite,
    Bounded(u64),
}

/// A Deserializer that reads bytes from a buffer.
///
/// This struct should rarely be used.
/// In most cases, prefer the `decode_from` function.
pub struct Deserializer<R, E>
    where R: Read,
          E: byteorder::ByteOrder
{
    reader: R,
    size_limit: SizeLimit,
    read: u64,
    _phantom: PhantomData<E>,
}

impl<R, E> Deserializer<R, E>
    where R: Read,
          E: byteorder::ByteOrder
{
    pub fn new(r: R, size_limit: SizeLimit) -> Deserializer<R, E> {
        Deserializer {
            reader: r,
            size_limit: size_limit,
            read: 0,
            _phantom: PhantomData,
        }
    }

    /// Returns the number of bytes read from the contained Reader.
    pub fn bytes_read(&self) -> u64 {
        self.read
    }

    fn read_variant_uint(&mut self) -> Result<usize> {
        let v: u8 = serde::Deserialize::deserialize(&mut *self)?;
        if v < 0xFF {
            Ok(v as usize)
        } else {
            let v: u32 = serde::Deserialize::deserialize(&mut *self)?;
            Ok(v as usize)
        }
    }

    fn read_bytes(&mut self, count: u64) -> Result<()> {
        self.read += count;
        match self.size_limit {
            SizeLimit::Infinite => Ok(()),
            SizeLimit::Bounded(x) if self.read <= x => Ok(()),
            SizeLimit::Bounded(_) => Err(ErrorKind::SizeLimit.into()),
        }
    }

    fn read_type<T>(&mut self) -> Result<()> {
        use std::mem::size_of;
        self.read_bytes(size_of::<T>() as u64)
    }

    fn read_str(&mut self) -> Result<String> {
        let len = self.read_variant_uint()? as u64;
        self.read_bytes(len)?;

        let mut buffer = Vec::new();
        self.reader.by_ref().take(len).read_to_end(&mut buffer)?;

        String::from_utf8(buffer).map_err(|err| {
            ErrorKind::InvalidEncoding {
                    desc: "error while decoding utf8 string",
                    detail: Some(format!("Deserialize error: {}", err)),
                }
                .into()
        })
    }
}

macro_rules! impl_nums {
    ($ty:ty, $dser_method:ident, $visitor_method:ident, $reader_method:ident) => {
        #[inline]
        fn $dser_method<V>(self, visitor: V) -> Result<V::Value>
            where V: serde::de::Visitor,
        {
            self.read_type::<$ty>()?;
            let value = self.reader.$reader_method::<E>()?;
            visitor.$visitor_method(value)
        }
    }
}


impl<'a, R, E> serde::Deserializer for &'a mut Deserializer<R, E>
    where R: Read,
          E: byteorder::ByteOrder
{
    type Error = Error;

    #[inline]
    fn deserialize<V>(self, _visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        panic!("bincode does not support Deserializer::deserialize.");
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        let value: u8 = serde::Deserialize::deserialize(self)?;
        match value {
            1 => visitor.visit_bool(true),
            0 => visitor.visit_bool(false),
            value => {
                Err(ErrorKind::InvalidEncoding {
                        desc: "invalid u8 when decoding bool",
                        detail: Some(format!("Expected 0 or 1, got {}", value)),
                    }
                    .into())
            }
        }
    }

    impl_nums!(u16, deserialize_u16, visit_u16, read_u16);
    impl_nums!(u32, deserialize_u32, visit_u32, read_u32);
    impl_nums!(u64, deserialize_u64, visit_u64, read_u64);
    impl_nums!(i16, deserialize_i16, visit_i16, read_i16);
    impl_nums!(i32, deserialize_i32, visit_i32, read_i32);
    impl_nums!(i64, deserialize_i64, visit_i64, read_i64);
    impl_nums!(f32, deserialize_f32, visit_f32, read_f32);
    impl_nums!(f64, deserialize_f64, visit_f64, read_f64);

    #[inline]
    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        self.read_type::<u8>()?;
        visitor.visit_u8(self.reader.read_u8()?)
    }

    #[inline]
    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        self.read_type::<i8>()?;
        visitor.visit_i8(self.reader.read_i8()?)
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        visitor.visit_unit()
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        use std::str;

        let error = ErrorKind::InvalidEncoding {
                desc: "Invalid char encoding",
                detail: None,
            }
            .into();

        let mut buf = [0];

        let _ = self.reader.read(&mut buf[..])?;
        let first_byte = buf[0];
        let width = utf8_char_width(first_byte);
        if width == 1 {
            return visitor.visit_char(first_byte as char);
        }
        if width == 0 {
            return Err(error);
        }

        let mut buf = [first_byte, 0, 0, 0];
        {
            let mut start = 1;
            while start < width {
                match self.reader.read(&mut buf[start..width])? {
                    n if n == width - start => break,
                    n if n < width - start => {
                        start += n;
                    }
                    _ => return Err(error),
                }
            }
        }

        let res = match str::from_utf8(&buf[..width]).ok() {
            Some(s) => Ok(s.chars().next().unwrap()),
            None => Err(error),
        }?;

        visitor.visit_char(res)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        visitor.visit_str(&self.read_str()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        visitor.visit_string(self.read_str()?)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_enum<V>(self,
                           _enum: &'static str,
                           _variants: &'static [&'static str],
                           visitor: V)
                           -> Result<V::Value>
        where V: serde::de::Visitor
    {
        impl<'a, R, E> serde::de::EnumVisitor for &'a mut Deserializer<R, E>
            where R: 'a + Read,
                  E: 'a + byteorder::ByteOrder
        {
            type Error = Error;
            type Variant = Self;

            fn visit_variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
                where V: serde::de::DeserializeSeed
            {
                let idx = self.read_variant_uint()? as u32;
                let val: Result<_> = seed.deserialize(idx.into_deserializer());
                Ok((val?, self))
            }
        }

        visitor.visit_enum(self)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        struct TupleVisitor<'a, R, E>(&'a mut Deserializer<R, E>)
            where R: 'a + Read,
                  E: 'a + byteorder::ByteOrder;

        impl<'a, 'b: 'a, R, E> serde::de::SeqVisitor for TupleVisitor<'a, R, E>
            where R: 'b + Read,
                  E: byteorder::ByteOrder
        {
            type Error = Error;

            fn visit_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
                where T: serde::de::DeserializeSeed
            {
                let value = serde::de::DeserializeSeed::deserialize(seed, &mut *self.0)?;
                Ok(Some(value))
            }
        }

        visitor.visit_seq(TupleVisitor(self))
    }

    fn deserialize_seq_fixed_size<V>(self, len: usize, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        struct SeqVisitor<'a, R, E>
            where R: 'a + Read,
                  E: 'a + byteorder::ByteOrder
        {
            deserializer: &'a mut Deserializer<R, E>,
            len: usize,
        }

        impl<'a, 'b: 'a, R, E> serde::de::SeqVisitor for SeqVisitor<'a, R, E>
            where R: 'b + Read,
                  E: byteorder::ByteOrder
        {
            type Error = Error;

            fn visit_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
                where T: serde::de::DeserializeSeed
            {
                if self.len > 0 {
                    self.len -= 1;
                    let value = serde::de::DeserializeSeed::deserialize(seed,
                                                                        &mut *self.deserializer)?;
                    Ok(Some(value))
                } else {
                    Ok(None)
                }
            }
        }

        visitor.visit_seq(SeqVisitor {
            deserializer: self,
            len: len,
        })
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        let value: u8 = serde::de::Deserialize::deserialize(&mut *self)?;
        match value {
            0 => visitor.visit_none(),
            1 => visitor.visit_some(&mut *self),
            _ => {
                Err(ErrorKind::InvalidEncoding {
                        desc: "invalid tag when decoding Option",
                        detail: Some(format!("Expected 0 or 1, got {}", value)),
                    }
                    .into())
            }
        }
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        let len = (&mut *self).read_variant_uint()?;
        self.deserialize_seq_fixed_size(len, visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        struct MapVisitor<'a, R, E>
            where R: 'a + Read,
                  E: 'a + byteorder::ByteOrder
        {
            deserializer: &'a mut Deserializer<R, E>,
            len: usize,
        }

        impl<'a, 'b: 'a, R, E> serde::de::MapVisitor for MapVisitor<'a, R, E>
            where R: 'b + Read,
                  E: byteorder::ByteOrder
        {
            type Error = Error;

            fn visit_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>>
                where K: serde::de::DeserializeSeed
            {
                if self.len > 0 {
                    self.len -= 1;
                    let key = serde::de::DeserializeSeed::deserialize(seed,
                                                                      &mut *self.deserializer)?;
                    Ok(Some(key))
                } else {
                    Ok(None)
                }
            }

            fn visit_value_seed<V>(&mut self, seed: V) -> Result<V::Value>
                where V: serde::de::DeserializeSeed
            {
                let value = serde::de::DeserializeSeed::deserialize(seed, &mut *self.deserializer)?;
                Ok(value)
            }
        }

        let len = serde::Deserialize::deserialize(&mut *self)?;

        visitor.visit_map(MapVisitor {
            deserializer: self,
            len: len,
        })
    }

    fn deserialize_struct<V>(self,
                             _name: &str,
                             fields: &'static [&'static str],
                             visitor: V)
                             -> Result<V::Value>
        where V: serde::de::Visitor
    {
        self.deserialize_tuple(fields.len(), visitor)
    }

    fn deserialize_struct_field<V>(self, _visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        let message = "bincode does not support Deserializer::deserialize_struct_field";
        Err(Error::custom(message))
    }

    fn deserialize_newtype_struct<V>(self, _name: &str, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        visitor.visit_unit()
    }

    fn deserialize_tuple_struct<V>(self,
                                   _name: &'static str,
                                   len: usize,
                                   visitor: V)
                                   -> Result<V::Value>
        where V: serde::de::Visitor
    {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        let message = "bincode does not support Deserializer::deserialize_ignored_any";
        Err(Error::custom(message))
    }
}

impl<'a, R, E> serde::de::VariantVisitor for &'a mut Deserializer<R, E>
    where R: Read,
          E: byteorder::ByteOrder
{
    type Error = Error;

    fn visit_unit(self) -> Result<()> {
        Ok(())
    }

    fn visit_newtype_seed<T>(self, seed: T) -> Result<T::Value>
        where T: serde::de::DeserializeSeed
    {
        serde::de::DeserializeSeed::deserialize(seed, self)
    }

    fn visit_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        serde::de::Deserializer::deserialize_tuple(self, len, visitor)
    }

    fn visit_struct<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
        where V: serde::de::Visitor
    {
        serde::de::Deserializer::deserialize_tuple(self, fields.len(), visitor)
    }
}

static UTF8_CHAR_WIDTH: [u8; 256] =
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
     1 /* 0x1F */, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
     1, 1, 1, 1, 1, 1, 1 /* 0x3F */, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
     1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1 /* 0x5F */, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
     1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1 /* 0x7F */, 0, 0, 0, 0, 0, 0, 0,
     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 /* 0x9F */, 0,
     0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
     0 /* 0xBF */, 0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
     2, 2, 2, 2, 2, 2, 2 /* 0xDF */, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
     3 /* 0xEF */, 4, 4, 4, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 /* 0xFF */];

fn utf8_char_width(b: u8) -> usize {
    UTF8_CHAR_WIDTH[b as usize] as usize
}