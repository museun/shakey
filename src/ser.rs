#![cfg_attr(debug_assertions, allow(dead_code, unused_variables,))]

use crate::Response;

pub fn get_fields<T>() -> Vec<&'static str>
where
    T: serde::Serialize + Response + Default,
{
    T::default().serialize(FieldSerializer).unwrap_or_default()
}

#[derive(Debug)]
enum Error {
    Unsupported,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported => f.write_str("unsupported type"),
        }
    }
}

impl std::error::Error for Error {}

impl serde::ser::Error for Error {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Self::Unsupported
    }
}

struct FieldCollector {
    fields: Vec<&'static str>,
}

impl serde::ser::SerializeStruct for FieldCollector {
    type Ok = Vec<&'static str>;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, key: &'static str, _: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize,
    {
        self.fields.push(key);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(self.fields)
    }
}

macro_rules! nope {
    ($($ident:ident => $ty:ty)*) => {
        $(
            fn $ident(self, _: $ty) -> Result<Self::Ok, Self::Error> {
                Err(Self::Error::Unsupported)
            }
        )*
    };
}

struct FieldSerializer;

impl serde::ser::Serializer for FieldSerializer {
    type Ok = Vec<&'static str>;
    type Error = Error;
    type SerializeStruct = FieldCollector;

    type SerializeSeq = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeMap = serde::ser::Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = serde::ser::Impossible<Self::Ok, Self::Error>;

    fn serialize_struct(
        self,
        _: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(FieldCollector {
            fields: Vec::with_capacity(len),
        })
    }

    nope! {
        serialize_bool  =>  bool
        serialize_i8    =>  i8
        serialize_i16   =>  i16
        serialize_i32   =>  i32
        serialize_i64   =>  i64
        serialize_u8    =>  u8
        serialize_u16   =>  u16
        serialize_u32   =>  u32
        serialize_u64   =>  u64
        serialize_f32   =>  f32
        serialize_f64   =>  f64
        serialize_char  =>  char
        serialize_str   =>  &str
        serialize_bytes =>  &[u8]
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::Unsupported)
    }

    fn serialize_some<T: ?Sized>(self, _: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(Self::Error::Unsupported)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::Unsupported)
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::Unsupported)
    }

    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Err(Self::Error::Unsupported)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _: &'static str,
        _: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(Self::Error::Unsupported)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize,
    {
        Err(Self::Error::Unsupported)
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(Self::Error::Unsupported)
    }

    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(Self::Error::Unsupported)
    }

    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(Self::Error::Unsupported)
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Self::Error::Unsupported)
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Self::Error::Unsupported)
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Self::Error::Unsupported)
    }
}
