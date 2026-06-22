use std::path::PathBuf;

use super::XDRSize;

fn pad(len: usize) -> usize {
    (4 - (len % 4)) % 4
}

fn opaque_size(len: usize) -> usize {
    len + pad(len)
}

fn variable_opaque_size(len: usize) -> usize {
    4 + opaque_size(len)
}

#[derive(XDRSize)]
struct Pair {
    a: u32,
    b: u64,
}

#[derive(XDRSize)]
struct Triple {
    a: u32,
    b: u32,
    c: u64,
}

#[derive(XDRSize)]
struct TuplePair(u32, u64);

#[derive(XDRSize)]
struct UnitStruct;

#[derive(XDRSize)]
struct Nested {
    pair: Pair,
    flag: bool,
}

#[derive(XDRSize)]
struct NestedTuple(TuplePair, bool);

#[derive(XDRSize)]
struct Generic<T> {
    value: T,
}

#[derive(XDRSize)]
struct GenericPair<T, U> {
    left: T,
    right: U,
}

#[derive(XDRSize)]
struct WithVec {
    items: Vec<u32>,
}

#[derive(XDRSize)]
struct WithString {
    value: String,
}

#[derive(XDRSize)]
struct WithBytes {
    value: Vec<u8>,
}

#[derive(XDRSize)]
struct WithArray {
    value: [u8; 5],
}

#[derive(XDRSize)]
struct WithOption {
    value: Option<u32>,
}

#[derive(XDRSize)]
struct WithResult {
    value: Result<u32, u64>,
}

#[derive(XDRSize)]
struct WithBox {
    value: Box<u32>,
}

#[derive(XDRSize)]
struct WithPath {
    value: PathBuf,
}

#[derive(XDRSize)]
enum Choice {
    Unit,
    Tuple(u32),
    Struct { x: u32, y: u64 },
}

#[derive(XDRSize)]
enum GenericChoice<T> {
    Empty,
    One(T),
    Two { value: T, flag: bool },
}

#[derive(XDRSize)]
enum ComplexEnum {
    A,
    B(Pair),
    C { nested: Nested, name: String },
}

#[test]
fn xdr_size_u32() {
    assert_eq!(0u32.xdr_size(), 4);
    assert_eq!(123u32.xdr_size(), 4);
}

#[test]
fn xdr_size_i32() {
    assert_eq!(0i32.xdr_size(), 4);
    assert_eq!((-123i32).xdr_size(), 4);
}

#[test]
fn xdr_size_u64() {
    assert_eq!(0u64.xdr_size(), 8);
    assert_eq!(123u64.xdr_size(), 8);
}

#[test]
fn xdr_size_usize() {
    assert_eq!(0usize.xdr_size(), 4);
    assert_eq!(123usize.xdr_size(), 4);
}

#[test]
fn xdr_size_bool() {
    assert_eq!(true.xdr_size(), 4);
    assert_eq!(false.xdr_size(), 4);
}

#[test]
fn xdr_size_byte_array_empty() {
    assert_eq!([0u8; 0].xdr_size(), 0);
}

#[test]
fn xdr_size_byte_array_1() {
    assert_eq!([0u8; 1].xdr_size(), 4);
}

#[test]
fn xdr_size_byte_array_2() {
    assert_eq!([0u8; 2].xdr_size(), 4);
}

#[test]
fn xdr_size_byte_array_3() {
    assert_eq!([0u8; 3].xdr_size(), 4);
}

#[test]
fn xdr_size_byte_array_4() {
    assert_eq!([0u8; 4].xdr_size(), 4);
}

#[test]
fn xdr_size_byte_array_5() {
    assert_eq!([0u8; 5].xdr_size(), 8);
}

#[test]
fn xdr_size_byte_array_7() {
    assert_eq!([0u8; 7].xdr_size(), 8);
}

#[test]
fn xdr_size_byte_array_8() {
    assert_eq!([0u8; 8].xdr_size(), 8);
}

#[test]
fn xdr_size_empty_vec_u8() {
    assert_eq!(Vec::<u8>::new().xdr_size(), variable_opaque_size(0));
}

#[test]
fn xdr_size_vec_u8_1() {
    assert_eq!(vec![1u8].xdr_size(), variable_opaque_size(1));
}

#[test]
fn xdr_size_vec_u8_2() {
    assert_eq!(vec![1u8, 2].xdr_size(), variable_opaque_size(2));
}

#[test]
fn xdr_size_vec_u8_3() {
    assert_eq!(vec![1u8, 2, 3].xdr_size(), variable_opaque_size(3));
}

#[test]
fn xdr_size_vec_u8_4() {
    assert_eq!(vec![1u8, 2, 3, 4].xdr_size(), variable_opaque_size(4));
}

#[test]
fn xdr_size_vec_u8_5() {
    assert_eq!(vec![1u8, 2, 3, 4, 5].xdr_size(), variable_opaque_size(5));
}

#[test]
fn xdr_size_string_empty() {
    assert_eq!("".to_string().xdr_size(), variable_opaque_size(0));
}

#[test]
fn xdr_size_string_1() {
    assert_eq!("a".to_string().xdr_size(), variable_opaque_size(1));
}

#[test]
fn xdr_size_string_3() {
    assert_eq!("abc".to_string().xdr_size(), variable_opaque_size(3));
}

#[test]
fn xdr_size_string_4() {
    assert_eq!("abcd".to_string().xdr_size(), variable_opaque_size(4));
}

#[test]
fn xdr_size_string_5() {
    assert_eq!("abcde".to_string().xdr_size(), variable_opaque_size(5));
}

#[test]
fn xdr_size_pathbuf() {
    let p = PathBuf::from("abc");
    assert_eq!(p.xdr_size(), variable_opaque_size(3));
}

#[test]
fn xdr_size_vec_u32_empty() {
    let v: Vec<u32> = vec![];
    assert_eq!(v.xdr_size(), 4);
}

#[test]
fn xdr_size_vec_u32_non_empty() {
    let v = vec![1u32, 2u32, 3u32];
    assert_eq!(v.xdr_size(), 4 + 3 * 4);
}

#[test]
fn xdr_size_vec_pair() {
    let v = vec![Pair { a: 1, b: 2 }, Pair { a: 3, b: 4 }];
    assert_eq!(v.xdr_size(), 4 + 2 * (4 + 8));
}

#[test]
fn xdr_size_option_none() {
    let v: Option<u32> = None;
    assert_eq!(v.xdr_size(), 4);
}

#[test]
fn xdr_size_option_some() {
    let v = Some(10u32);
    assert_eq!(v.xdr_size(), 4 + 4);
}

#[test]
fn xdr_size_result_ok() {
    let v: Result<u32, u64> = Ok(10u32);
    assert_eq!(v.xdr_size(), 4 + 4);
}

#[test]
fn xdr_size_result_err() {
    let v: Result<u32, u64> = Err(10u64);
    assert_eq!(v.xdr_size(), 4 + 8);
}

#[test]
fn xdr_size_box_u32() {
    let v = Box::new(123u32);
    assert_eq!(v.xdr_size(), 4);
}

#[test]
fn derive_named_struct_pair() {
    let v = Pair { a: 1, b: 2 };
    assert_eq!(v.xdr_size(), 4 + 8);
}

#[test]
fn derive_named_struct_triple() {
    let v = Triple { a: 1, b: 2, c: 3 };
    assert_eq!(v.xdr_size(), 4 + 4 + 8);
}

#[test]
fn derive_tuple_struct() {
    let v = TuplePair(1, 2);
    assert_eq!(v.xdr_size(), 4 + 8);
}

#[test]
fn derive_unit_struct() {
    let v = UnitStruct;
    assert_eq!(v.xdr_size(), 0);
}

#[test]
fn derive_nested_struct() {
    let v = Nested { pair: Pair { a: 1, b: 2 }, flag: true };
    assert_eq!(v.xdr_size(), (4 + 8) + 4);
}

#[test]
fn derive_nested_tuple_struct() {
    let v = NestedTuple(TuplePair(1, 2), true);
    assert_eq!(v.xdr_size(), (4 + 8) + 4);
}

#[test]
fn derive_struct_with_vec() {
    let v = WithVec { items: vec![1u32, 2u32, 3u32] };
    assert_eq!(v.xdr_size(), 4 + 3 * 4);
}

#[test]
fn derive_struct_with_string() {
    let s = "hello".to_string();
    let v = WithString { value: s.clone() };
    assert_eq!(v.xdr_size(), variable_opaque_size(s.len()));
}

#[test]
fn derive_struct_with_bytes() {
    let bytes = vec![1u8, 2, 3, 4, 5];
    let v = WithBytes { value: bytes.clone() };
    assert_eq!(v.xdr_size(), variable_opaque_size(bytes.len()));
}

#[test]
fn derive_struct_with_array() {
    let v = WithArray { value: [1u8; 5] };
    assert_eq!(v.xdr_size(), 8);
}

#[test]
fn derive_struct_with_option() {
    let v = WithOption { value: Some(5) };
    assert_eq!(v.xdr_size(), 4 + 4);
}

#[test]
fn derive_struct_with_result() {
    let v = WithResult { value: Ok(5u32) };
    assert_eq!(v.xdr_size(), 4 + 4);
}

#[test]
fn derive_struct_with_box() {
    let v = WithBox { value: Box::new(10u32) };
    assert_eq!(v.xdr_size(), 4);
}

#[test]
fn derive_struct_with_path() {
    let path = PathBuf::from("abcde");
    let v = WithPath { value: path.clone() };
    assert_eq!(v.xdr_size(), variable_opaque_size(5));
}

#[test]
fn derive_generic_struct_u32() {
    let v = Generic { value: 42u32 };
    assert_eq!(v.xdr_size(), 4);
}

#[test]
fn derive_generic_struct_pair() {
    let v = Generic { value: Pair { a: 1, b: 2 } };
    assert_eq!(v.xdr_size(), 4 + 8);
}

#[test]
fn derive_generic_pair() {
    let v = GenericPair { left: 10u32, right: 20u64 };
    assert_eq!(v.xdr_size(), 4 + 8);
}

#[test]
fn derive_enum_unit_variant() {
    assert_eq!(Choice::Unit.xdr_size(), 4);
}

#[test]
fn derive_enum_tuple_variant() {
    assert_eq!(Choice::Tuple(5).xdr_size(), 4 + 4);
}

#[test]
fn derive_enum_struct_variant() {
    assert_eq!(Choice::Struct { x: 1, y: 2 }.xdr_size(), 4 + 4 + 8);
}

#[test]
fn derive_generic_enum_empty() {
    let v: GenericChoice<u32> = GenericChoice::Empty;
    assert_eq!(v.xdr_size(), 4);
}

#[test]
fn derive_generic_enum_one() {
    let v = GenericChoice::One(7u32);
    assert_eq!(v.xdr_size(), 4 + 4);
}

#[test]
fn derive_generic_enum_two() {
    let v = GenericChoice::Two { value: 7u32, flag: true };
    assert_eq!(v.xdr_size(), 4 + 4 + 4);
}

#[test]
fn derive_complex_enum_a() {
    assert_eq!(ComplexEnum::A.xdr_size(), 4);
}

#[test]
fn derive_complex_enum_b() {
    let v = ComplexEnum::B(Pair { a: 1, b: 2 });
    assert_eq!(v.xdr_size(), 4 + (4 + 8));
}

#[test]
fn derive_complex_enum_c() {
    let nested = Nested { pair: Pair { a: 1, b: 2 }, flag: true };
    let name = "abc".to_string();

    let expected_nested = (4 + 8) + 4;
    let expected_name = variable_opaque_size(name.len());

    let v = ComplexEnum::C { nested, name };
    assert_eq!(v.xdr_size(), 4 + expected_nested + expected_name);
}

#[test]
fn nested_option_vec_pair() {
    let v = Some(vec![Pair { a: 1, b: 2 }, Pair { a: 3, b: 4 }]);
    assert_eq!(v.xdr_size(), 4 + 4 + 2 * (4 + 8));
}

#[test]
fn nested_result_pair_u32_ok() {
    let v: Result<Pair, u32> = Ok(Pair { a: 1, b: 2 });
    assert_eq!(v.xdr_size(), 4 + (4 + 8));
}

#[test]
fn nested_result_pair_u32_err() {
    let v: Result<Pair, u32> = Err(5u32);
    assert_eq!(v.xdr_size(), 4 + 4);
}

#[test]
fn nested_derive_inside_generic() {
    let v = Generic { value: Nested { pair: Pair { a: 1, b: 2 }, flag: true } };

    assert_eq!(v.xdr_size(), (4 + 8) + 4);
}

#[test]
fn vec_of_generic_structs() {
    let v = vec![Generic { value: 1u32 }, Generic { value: 2u32 }, Generic { value: 3u32 }];

    assert_eq!(v.xdr_size(), 4 + 3 * 4);
}

#[test]
fn padding_helper() {
    assert_eq!(pad(0), 0);
    assert_eq!(pad(1), 3);
    assert_eq!(pad(2), 2);
    assert_eq!(pad(3), 1);
    assert_eq!(pad(4), 0);
    assert_eq!(pad(5), 3);
}

#[test]
fn opaque_size_helper() {
    assert_eq!(opaque_size(0), 0);
    assert_eq!(opaque_size(1), 4);
    assert_eq!(opaque_size(2), 4);
    assert_eq!(opaque_size(3), 4);
    assert_eq!(opaque_size(4), 4);
    assert_eq!(opaque_size(5), 8);
}

#[test]
fn variable_opaque_size_helper() {
    assert_eq!(variable_opaque_size(0), 4);
    assert_eq!(variable_opaque_size(1), 8);
    assert_eq!(variable_opaque_size(4), 8);
    assert_eq!(variable_opaque_size(5), 12);
}
