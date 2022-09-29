// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Logic for serde-compatible deserialization.
use crate::{types::Value, Error};
use serde::{
    de::{self, DeserializeSeed, Visitor},
    forward_to_deserialize_any, Deserialize,
};
use std::{
    collections::{
        hash_map::{Keys, Values},
        HashMap,
    },
    slice::Iter,
};

pub struct Deserializer<'de> {
    input: &'de Value,
}

struct SeqDeserializer<'de> {
    input: Iter<'de, Value>,
}

struct MapDeserializer<'de> {
    input_keys: Keys<'de, String, Value>,
    input_values: Values<'de, String, Value>,
}

struct RecordDeserializer<'de> {
    input: Iter<'de, (String, Value)>,
    value: Option<&'de Value>,
}

pub struct EnumUnitDeserializer<'a> {
    input: &'a str,
}

pub struct EnumDeserializer<'de> {
    input: &'de [(String, Value)],
}

impl<'de> Deserializer<'de> {
    pub fn new(input: &'de Value) -> Self {
        Deserializer { input }
    }
}

impl<'de> SeqDeserializer<'de> {
    pub fn new(input: &'de [Value]) -> Self {
        SeqDeserializer {
            input: input.iter(),
        }
    }
}

impl<'de> MapDeserializer<'de> {
    pub fn new(input: &'de HashMap<String, Value>) -> Self {
        MapDeserializer {
            input_keys: input.keys(),
            input_values: input.values(),
        }
    }
}

impl<'de> RecordDeserializer<'de> {
    pub fn new(input: &'de [(String, Value)]) -> Self {
        RecordDeserializer {
            input: input.iter(),
            value: None,
        }
    }
}

impl<'a> EnumUnitDeserializer<'a> {
    pub fn new(input: &'a str) -> Self {
        EnumUnitDeserializer { input }
    }
}

impl<'de> EnumDeserializer<'de> {
    pub fn new(input: &'de [(String, Value)]) -> Self {
        EnumDeserializer { input }
    }
}

impl<'de> de::EnumAccess<'de> for EnumUnitDeserializer<'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        Ok((
            seed.deserialize(StringDeserializer {
                input: self.input.to_owned(),
            })?,
            self,
        ))
    }
}

impl<'de> de::VariantAccess<'de> for EnumUnitDeserializer<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, _seed: T) -> Result<T::Value, Error>
    where
        T: DeserializeSeed<'de>,
    {
        Err(de::Error::custom("Unexpected Newtype variant"))
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        Err(de::Error::custom("Unexpected tuple variant"))
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        Err(de::Error::custom("Unexpected struct variant"))
    }
}

impl<'de> de::EnumAccess<'de> for EnumDeserializer<'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        self.input.first().map_or(
            Err(de::Error::custom("A record must have a least one field")),
            |item| match (item.0.as_ref(), &item.1) {
                ("type", Value::String(x)) | ("type", Value::Enum(_, x)) => Ok((
                    seed.deserialize(StringDeserializer {
                        input: x.to_owned(),
                    })?,
                    self,
                )),
                (field, Value::String(_)) => Err(de::Error::custom(format!(
                    "Expected first field named 'type': got '{field}' instead"
                ))),
                (_, _) => Err(de::Error::custom(
                    "Expected first field of type String or Enum for the type name".to_string(),
                )),
            },
        )
    }
}

impl<'de> de::VariantAccess<'de> for EnumDeserializer<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Error>
    where
        T: DeserializeSeed<'de>,
    {
        self.input.get(1).map_or(
            Err(de::Error::custom(
                "Expected a newtype variant, got nothing instead.",
            )),
            |item| seed.deserialize(&Deserializer::new(&item.1)),
        )
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.input.get(1).map_or(
            Err(de::Error::custom(
                "Expected a tuple variant, got nothing instead.",
            )),
            |item| de::Deserializer::deserialize_seq(&Deserializer::new(&item.1), visitor),
        )
    }

    fn struct_variant<V>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.input.get(1).map_or(
            Err(de::Error::custom("Expected a struct variant, got nothing")),
            |item| {
                de::Deserializer::deserialize_struct(
                    &Deserializer::new(&item.1),
                    "",
                    fields,
                    visitor,
                )
            },
        )
    }
}

impl<'a, 'de> de::Deserializer<'de> for &'a Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.input {
            Value::Null => visitor.visit_unit(),
            &Value::Boolean(b) => visitor.visit_bool(b),
            Value::Int(i) | Value::Date(i) | Value::TimeMillis(i) => visitor.visit_i32(*i),
            Value::Long(i)
            | Value::TimeMicros(i)
            | Value::TimestampMillis(i)
            | Value::TimestampMicros(i) => visitor.visit_i64(*i),
            &Value::Float(f) => visitor.visit_f32(f),
            &Value::Double(d) => visitor.visit_f64(d),
            Value::Union(_i, u) => match **u {
                Value::Null => visitor.visit_unit(),
                Value::Boolean(b) => visitor.visit_bool(b),
                Value::Int(i) => visitor.visit_i32(i),
                Value::Long(i)
                | Value::TimeMicros(i)
                | Value::TimestampMillis(i)
                | Value::TimestampMicros(i) => visitor.visit_i64(i),
                Value::Float(f) => visitor.visit_f32(f),
                Value::Double(d) => visitor.visit_f64(d),
                Value::Record(ref fields) => visitor.visit_map(RecordDeserializer::new(fields)),
                Value::Array(ref fields) => visitor.visit_seq(SeqDeserializer::new(fields)),
                Value::String(ref s) => visitor.visit_borrowed_str(s),
                Value::Map(ref items) => visitor.visit_map(MapDeserializer::new(items)),
                _ => Err(de::Error::custom(format!(
                    "unsupported union: {:?}",
                    self.input
                ))),
            },
            Value::Record(ref fields) => visitor.visit_map(RecordDeserializer::new(fields)),
            Value::Array(ref fields) => visitor.visit_seq(SeqDeserializer::new(fields)),
            Value::String(ref s) => visitor.visit_borrowed_str(s),
            Value::Map(ref items) => visitor.visit_map(MapDeserializer::new(items)),
            value => Err(de::Error::custom(format!(
                "incorrect value of type: {:?}",
                crate::schema::SchemaKind::from(value)
            ))),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
    }

    fn deserialize_char<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(de::Error::custom("avro does not support char"))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_borrowed_str(s),
            Value::Bytes(ref bytes) | Value::Fixed(_, ref bytes) => ::std::str::from_utf8(bytes)
                .map_err(|e| de::Error::custom(e.to_string()))
                .and_then(|s| visitor.visit_borrowed_str(s)),
            Value::Uuid(ref u) => visitor.visit_str(&u.to_string()),
            _ => Err(de::Error::custom("not a string|bytes|fixed")),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_borrowed_str(s),
            Value::Bytes(ref bytes) | Value::Fixed(_, ref bytes) => {
                String::from_utf8(bytes.to_owned())
                    .map_err(|e| de::Error::custom(e.to_string()))
                    .and_then(|s| visitor.visit_string(s))
            }
            Value::Union(_i, ref x) => match **x {
                Value::String(ref s) => visitor.visit_borrowed_str(s),
                _ => Err(de::Error::custom("not a string|bytes|fixed")),
            },
            _ => Err(de::Error::custom("not a string|bytes|fixed")),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_bytes(s.as_bytes()),
            Value::Bytes(ref bytes) | Value::Fixed(_, ref bytes) => visitor.visit_bytes(bytes),
            Value::Uuid(ref u) => visitor.visit_bytes(u.as_bytes()),
            _ => Err(de::Error::custom("not a string|bytes|fixed")),
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_byte_buf(s.clone().into_bytes()),
            Value::Bytes(ref bytes) | Value::Fixed(_, ref bytes) => {
                visitor.visit_byte_buf(bytes.to_owned())
            }
            _ => Err(de::Error::custom("not a string|bytes|fixed")),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::Union(_i, ref inner) if inner.as_ref() == &Value::Null => visitor.visit_none(),
            Value::Union(_i, ref inner) => visitor.visit_some(&Deserializer::new(inner)),
            _ => Err(de::Error::custom("not a union")),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::Null => visitor.visit_unit(),
            Value::Union(_i, ref x) => match **x {
                Value::Null => visitor.visit_unit(),
                _ => Err(de::Error::custom("not a null")),
            },
            _ => Err(de::Error::custom("not a null")),
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::Array(ref items) => visitor.visit_seq(SeqDeserializer::new(items)),
            Value::Union(_i, ref inner) => match **inner {
                Value::Array(ref items) => visitor.visit_seq(SeqDeserializer::new(items)),
                _ => Err(de::Error::custom("not an array")),
            },
            _ => Err(de::Error::custom("not an array")),
        }
    }

    fn deserialize_tuple<V>(self, _: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _: &'static str,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::Map(ref items) => visitor.visit_map(MapDeserializer::new(items)),
            Value::Record(ref fields) => visitor.visit_map(RecordDeserializer::new(fields)),
            _ => Err(de::Error::custom(format_args!(
                "Expected a record or a map. Got: {:?}",
                &self.input
            ))),
        }
    }

    fn deserialize_struct<V>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::Record(ref fields) => visitor.visit_map(RecordDeserializer::new(fields)),
            Value::Union(_i, ref inner) => match **inner {
                Value::Record(ref fields) => visitor.visit_map(RecordDeserializer::new(fields)),
                _ => Err(de::Error::custom("not a record")),
            },
            _ => Err(de::Error::custom("not a record")),
        }
    }

    fn deserialize_enum<V>(
        self,
        _: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            // This branch can be anything...
            Value::Record(ref fields) => visitor.visit_enum(EnumDeserializer::new(fields)),
            // This has to be a unit Enum
            Value::Enum(_index, ref field) => visitor.visit_enum(EnumUnitDeserializer::new(field)),
            _ => Err(de::Error::custom("not an enum")),
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

impl<'de> de::SeqAccess<'de> for SeqDeserializer<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.input.next() {
            Some(item) => seed.deserialize(&Deserializer::new(item)).map(Some),
            None => Ok(None),
        }
    }
}

impl<'de> de::MapAccess<'de> for MapDeserializer<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.input_keys.next() {
            Some(key) => seed
                .deserialize(StringDeserializer {
                    input: (*key).clone(),
                })
                .map(Some),
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        match self.input_values.next() {
            Some(value) => seed.deserialize(&Deserializer::new(value)),
            None => Err(de::Error::custom("should not happen - too many values")),
        }
    }
}

impl<'de> de::MapAccess<'de> for RecordDeserializer<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.input.next() {
            Some(item) => {
                let (ref field, ref value) = *item;
                self.value = Some(value);
                seed.deserialize(StringDeserializer {
                    input: field.clone(),
                })
                .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => seed.deserialize(&Deserializer::new(value)),
            None => Err(de::Error::custom("should not happen - too many values")),
        }
    }
}

#[derive(Clone)]
struct StringDeserializer {
    input: String,
}

impl<'de> de::Deserializer<'de> for StringDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.input)
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string unit option
        seq bytes byte_buf map unit_struct newtype_struct
        tuple_struct struct tuple enum identifier ignored_any
    }
}

/// Interpret a `Value` as an instance of type `D`.
///
/// This conversion can fail if the structure of the `Value` does not match the
/// structure expected by `D`.
pub fn from_value<'de, D: Deserialize<'de>>(value: &'de Value) -> Result<D, Error> {
    let de = Deserializer::new(value);
    D::deserialize(&de)
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde::Serialize;
    use uuid::Uuid;

    use super::*;

    #[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
    struct Test {
        a: i64,
        b: String,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct TestInner {
        a: Test,
        b: i32,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct TestUnitExternalEnum {
        a: UnitExternalEnum,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    enum UnitExternalEnum {
        Val1,
        Val2,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct TestUnitInternalEnum {
        a: UnitInternalEnum,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(tag = "t")]
    enum UnitInternalEnum {
        Val1,
        Val2,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct TestUnitAdjacentEnum {
        a: UnitAdjacentEnum,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(tag = "t", content = "v")]
    enum UnitAdjacentEnum {
        Val1,
        Val2,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct TestUnitUntaggedEnum {
        a: UnitUntaggedEnum,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(untagged)]
    enum UnitUntaggedEnum {
        Val1,
        Val2,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct TestNullExternalEnum {
        a: NullExternalEnum,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    enum NullExternalEnum {
        Val1(()),
        Val2(u64),
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestSingleValueExternalEnum {
        a: SingleValueExternalEnum,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum SingleValueExternalEnum {
        Double(f64),
        String(String),
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestStructExternalEnum {
        a: StructExternalEnum,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum StructExternalEnum {
        Val1 { x: f32, y: f32 },
        Val2 { x: f32, y: f32 },
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestTupleExternalEnum {
        a: TupleExternalEnum,
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    enum TupleExternalEnum {
        Val1(f32, f32),
        Val2(f32, f32, f32),
    }

    #[test]
    fn test_from_value() {
        let test = Value::Record(vec![
            ("a".to_owned(), Value::Long(27)),
            ("b".to_owned(), Value::String("foo".to_owned())),
        ]);
        let expected = Test {
            a: 27,
            b: "foo".to_owned(),
        };
        let final_value: Test = from_value(&test).unwrap();
        assert_eq!(final_value, expected);

        let test_inner = Value::Record(vec![
            (
                "a".to_owned(),
                Value::Record(vec![
                    ("a".to_owned(), Value::Long(27)),
                    ("b".to_owned(), Value::String("foo".to_owned())),
                ]),
            ),
            ("b".to_owned(), Value::Int(35)),
        ]);

        let expected_inner = TestInner { a: expected, b: 35 };
        let final_value: TestInner = from_value(&test_inner).unwrap();
        assert_eq!(final_value, expected_inner)
    }
    #[test]
    fn test_from_value_unit_enum() {
        let expected = TestUnitExternalEnum {
            a: UnitExternalEnum::Val1,
        };

        let test = Value::Record(vec![("a".to_owned(), Value::Enum(0, "Val1".to_owned()))]);
        let final_value: TestUnitExternalEnum = from_value(&test).unwrap();
        assert_eq!(
            final_value, expected,
            "Error deserializing unit external enum"
        );

        let expected = TestUnitInternalEnum {
            a: UnitInternalEnum::Val1,
        };

        let test = Value::Record(vec![(
            "a".to_owned(),
            Value::Record(vec![("t".to_owned(), Value::String("Val1".to_owned()))]),
        )]);
        let final_value: TestUnitInternalEnum = from_value(&test).unwrap();
        assert_eq!(
            final_value, expected,
            "Error deserializing unit internal enum"
        );
        let expected = TestUnitAdjacentEnum {
            a: UnitAdjacentEnum::Val1,
        };

        let test = Value::Record(vec![(
            "a".to_owned(),
            Value::Record(vec![("t".to_owned(), Value::String("Val1".to_owned()))]),
        )]);
        let final_value: TestUnitAdjacentEnum = from_value(&test).unwrap();
        assert_eq!(
            final_value, expected,
            "Error deserializing unit adjacent enum"
        );
        let expected = TestUnitUntaggedEnum {
            a: UnitUntaggedEnum::Val1,
        };

        let test = Value::Record(vec![("a".to_owned(), Value::Null)]);
        let final_value: TestUnitUntaggedEnum = from_value(&test).unwrap();
        assert_eq!(
            final_value, expected,
            "Error deserializing unit untagged enum"
        );
    }

    #[test]
    fn test_from_value_null_enum() {
        let expected = TestNullExternalEnum {
            a: NullExternalEnum::Val1(()),
        };

        let test = Value::Record(vec![(
            "a".to_owned(),
            Value::Record(vec![
                ("type".to_owned(), Value::String("Val1".to_owned())),
                ("value".to_owned(), Value::Union(0, Box::new(Value::Null))),
            ]),
        )]);
        let final_value: TestNullExternalEnum = from_value(&test).unwrap();
        assert_eq!(
            final_value, expected,
            "Error deserializing null external enum"
        );
    }

    #[test]
    fn test_from_value_single_value_enum() {
        let expected = TestSingleValueExternalEnum {
            a: SingleValueExternalEnum::Double(64.0),
        };

        let test = Value::Record(vec![(
            "a".to_owned(),
            Value::Record(vec![
                ("type".to_owned(), Value::String("Double".to_owned())),
                (
                    "value".to_owned(),
                    Value::Union(1, Box::new(Value::Double(64.0))),
                ),
            ]),
        )]);
        let final_value: TestSingleValueExternalEnum = from_value(&test).unwrap();
        assert_eq!(
            final_value, expected,
            "Error deserializing single value external enum(union)"
        );
    }

    #[test]
    fn test_from_value_struct_enum() {
        let expected = TestStructExternalEnum {
            a: StructExternalEnum::Val1 { x: 1.0, y: 2.0 },
        };

        let test = Value::Record(vec![(
            "a".to_owned(),
            Value::Record(vec![
                ("type".to_owned(), Value::String("Val1".to_owned())),
                (
                    "value".to_owned(),
                    Value::Union(
                        0,
                        Box::new(Value::Record(vec![
                            ("x".to_owned(), Value::Float(1.0)),
                            ("y".to_owned(), Value::Float(2.0)),
                        ])),
                    ),
                ),
            ]),
        )]);
        let final_value: TestStructExternalEnum = from_value(&test).unwrap();
        assert_eq!(
            final_value, expected,
            "error deserializing struct external enum(union)"
        );
    }

    #[test]
    fn test_avro_3692_from_value_struct_flatten() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct S1 {
            f1: String,
            #[serde(flatten)]
            inner: S2,
        }
        #[derive(Deserialize, PartialEq, Debug)]
        struct S2 {
            f2: String,
        }
        let expected = S1 {
            f1: "Hello".to_owned(),
            inner: S2 {
                f2: "World".to_owned(),
            },
        };

        let test = Value::Record(vec![
            ("f1".to_owned(), "Hello".into()),
            ("f2".to_owned(), "World".into()),
        ]);
        let final_value: S1 = from_value(&test).unwrap();
        assert_eq!(final_value, expected);
    }

    #[test]
    fn test_from_value_tuple_enum() {
        let expected = TestTupleExternalEnum {
            a: TupleExternalEnum::Val1(1.0, 2.0),
        };

        let test = Value::Record(vec![(
            "a".to_owned(),
            Value::Record(vec![
                ("type".to_owned(), Value::String("Val1".to_owned())),
                (
                    "value".to_owned(),
                    Value::Union(
                        0,
                        Box::new(Value::Array(vec![Value::Float(1.0), Value::Float(2.0)])),
                    ),
                ),
            ]),
        )]);
        let final_value: TestTupleExternalEnum = from_value(&test).unwrap();
        assert_eq!(
            final_value, expected,
            "error serializing tuple external enum(union)"
        );
    }

    type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

    #[test]
    fn test_date() -> TestResult<()> {
        let raw_value = 1;
        let value = Value::Date(raw_value);
        let result = crate::from_value::<i32>(&value)?;
        assert_eq!(result, raw_value);
        Ok(())
    }

    #[test]
    fn test_time_millis() -> TestResult<()> {
        let raw_value = 1;
        let value = Value::TimeMillis(raw_value);
        let result = crate::from_value::<i32>(&value)?;
        assert_eq!(result, raw_value);
        Ok(())
    }

    #[test]
    fn test_time_micros() -> TestResult<()> {
        let raw_value = 1;
        let value = Value::TimeMicros(raw_value);
        let result = crate::from_value::<i64>(&value)?;
        assert_eq!(result, raw_value);
        Ok(())
    }

    #[test]
    fn test_timestamp_millis() -> TestResult<()> {
        let raw_value = 1;
        let value = Value::TimestampMillis(raw_value);
        let result = crate::from_value::<i64>(&value)?;
        assert_eq!(result, raw_value);
        Ok(())
    }

    #[test]
    fn test_timestamp_micros() -> TestResult<()> {
        let raw_value = 1;
        let value = Value::TimestampMicros(raw_value);
        let result = crate::from_value::<i64>(&value)?;
        assert_eq!(result, raw_value);
        Ok(())
    }

    #[test]
    fn test_from_value_uuid_str() -> TestResult<()> {
        let raw_value = "9ec535ff-3e2a-45bd-91d3-0a01321b5a49";
        let value = Value::Uuid(Uuid::parse_str(raw_value).unwrap());
        let result = crate::from_value::<Uuid>(&value)?;
        assert_eq!(result.to_string(), raw_value);
        Ok(())
    }

    #[test]
    fn test_from_value_uuid_slice() -> TestResult<()> {
        let raw_value = &[4, 54, 67, 12, 43, 2, 2, 76, 32, 50, 87, 5, 1, 33, 43, 87];
        let value = Value::Uuid(Uuid::from_slice(raw_value)?);
        let result = crate::from_value::<Uuid>(&value)?;
        assert_eq!(result.as_bytes(), raw_value);
        Ok(())
    }

    #[test]
    fn test_from_value_with_union() -> TestResult<()> {
        // AVRO-3232 test for deserialize_any on missing fields on the destination struct:
        // Error: DeserializeValue("Unsupported union")
        // Error: DeserializeValue("incorrect value of type: String")
        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct RecordInUnion {
            record_in_union: i32,
        }

        #[derive(Debug, Deserialize, PartialEq, Eq)]
        struct StructWithMissingFields {
            a_string: String,
            a_record: Option<RecordInUnion>,
            an_array: Option<[bool; 2]>,
            a_union_map: Option<HashMap<String, i64>>,
        }

        let raw_map: HashMap<String, i64> = [
            ("long_one".to_string(), 1),
            ("long_two".to_string(), 2),
            ("long_three".to_string(), 3),
            ("time_micros_a".to_string(), 123),
            ("timestamp_millis_b".to_string(), 234),
            ("timestamp_micros_c".to_string(), 345),
        ]
        .iter()
        .cloned()
        .collect();

        let value_map = raw_map
            .iter()
            .map(|(k, v)| match k {
                key if key.starts_with("long_") => (k.clone(), Value::Long(*v)),
                key if key.starts_with("time_micros_") => (k.clone(), Value::TimeMicros(*v)),
                key if key.starts_with("timestamp_millis_") => {
                    (k.clone(), Value::TimestampMillis(*v))
                }
                key if key.starts_with("timestamp_micros_") => {
                    (k.clone(), Value::TimestampMicros(*v))
                }
                _ => unreachable!("unexpected key: {:?}", k),
            })
            .collect();

        let record = Value::Record(vec![
            (
                "a_string".to_string(),
                Value::String("a valid message field".to_string()),
            ),
            (
                "a_non_existing_string".to_string(),
                Value::String("a string".to_string()),
            ),
            (
                "a_union_string".to_string(),
                Value::Union(0, Box::new(Value::String("a union string".to_string()))),
            ),
            (
                "a_union_long".to_string(),
                Value::Union(0, Box::new(Value::Long(412))),
            ),
            (
                "a_union_long".to_string(),
                Value::Union(0, Box::new(Value::Long(412))),
            ),
            (
                "a_time_micros".to_string(),
                Value::Union(0, Box::new(Value::TimeMicros(123))),
            ),
            (
                "a_non_existing_time_micros".to_string(),
                Value::Union(0, Box::new(Value::TimeMicros(-123))),
            ),
            (
                "a_timestamp_millis".to_string(),
                Value::Union(0, Box::new(Value::TimestampMillis(234))),
            ),
            (
                "a_non_existing_timestamp_millis".to_string(),
                Value::Union(0, Box::new(Value::TimestampMillis(-234))),
            ),
            (
                "a_timestamp_micros".to_string(),
                Value::Union(0, Box::new(Value::TimestampMicros(345))),
            ),
            (
                "a_non_existing_timestamp_micros".to_string(),
                Value::Union(0, Box::new(Value::TimestampMicros(-345))),
            ),
            (
                "a_record".to_string(),
                Value::Union(
                    0,
                    Box::new(Value::Record(vec![(
                        "record_in_union".to_string(),
                        Value::Int(-2),
                    )])),
                ),
            ),
            (
                "a_non_existing_record".to_string(),
                Value::Union(
                    0,
                    Box::new(Value::Record(vec![("blah".to_string(), Value::Int(-22))])),
                ),
            ),
            (
                "an_array".to_string(),
                Value::Union(
                    0,
                    Box::new(Value::Array(vec![
                        Value::Boolean(true),
                        Value::Boolean(false),
                    ])),
                ),
            ),
            (
                "a_non_existing_array".to_string(),
                Value::Union(
                    0,
                    Box::new(Value::Array(vec![
                        Value::Boolean(false),
                        Value::Boolean(true),
                    ])),
                ),
            ),
            (
                "a_union_map".to_string(),
                Value::Union(0, Box::new(Value::Map(value_map))),
            ),
            (
                "a_non_existing_union_map".to_string(),
                Value::Union(0, Box::new(Value::Map(HashMap::new()))),
            ),
        ]);

        let deserialized: StructWithMissingFields = crate::from_value(&record)?;
        let reference = StructWithMissingFields {
            a_string: "a valid message field".to_string(),
            a_record: Some(RecordInUnion {
                record_in_union: -2,
            }),
            an_array: Some([true, false]),
            a_union_map: Some(raw_map),
        };
        assert_eq!(deserialized, reference);
        Ok(())
    }

    #[test]
    fn test_struct_fixed_field_avro_3631() {
        #[derive(Debug, Serialize, Deserialize)]
        struct TestStructFixedField {
            field: [u8; 6],
        }

        let value = Value::Record(vec![(
            "field".to_string(),
            Value::Fixed(6, vec![0, 0, 0, 0, 0, 0]),
        )]);
        let _deserialized: TestStructFixedField = crate::from_value(&value).unwrap();
    }
}
