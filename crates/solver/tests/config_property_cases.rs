// Complete adapted ConfigUtilsTest cases and its bean/enum fixture.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::str::FromStr;
use timetuck::*;
#[derive(Default)]
struct Bean {
    primitive_boolean: bool,
    object_boolean: Option<bool>,
    primitive_int: i32,
    object_integer: Option<i32>,
    primitive_long: i64,
    object_long: Option<i64>,
    primitive_float: f32,
    object_float: Option<f32>,
    primitive_double: f64,
    object_double: Option<f64>,
    big_decimal: Option<Decimal>,
    string: Option<String>,
    kind: Option<Kind>,
}
#[derive(Debug, PartialEq)]
enum Kind {
    Alpha,
    Beta,
    Gamma,
}
impl FromStr for Kind {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ALPHA" => Ok(Self::Alpha),
            "BETA" => Ok(Self::Beta),
            "GAMMA" => Ok(Self::Gamma),
            _ => Err("unknown enum value"),
        }
    }
}
fn registry() -> ConfigPropertyRegistry<Bean> {
    let mut r = ConfigPropertyRegistry::<Bean>::default();
    r.register("primitiveBoolean", |b, v: bool| b.primitive_boolean = v)
        .unwrap();
    r.register("objectBoolean", |b, v: bool| b.object_boolean = Some(v))
        .unwrap();
    r.register("primitiveInt", |b, v: i32| b.primitive_int = v)
        .unwrap();
    r.register("objectInteger", |b, v: i32| b.object_integer = Some(v))
        .unwrap();
    r.register("primitiveLong", |b, v: i64| b.primitive_long = v)
        .unwrap();
    r.register("objectLong", |b, v: i64| b.object_long = Some(v))
        .unwrap();
    r.register("primitiveFloat", |b, v: f32| b.primitive_float = v)
        .unwrap();
    r.register("objectFloat", |b, v: f32| b.object_float = Some(v))
        .unwrap();
    r.register("primitiveDouble", |b, v: f64| b.primitive_double = v)
        .unwrap();
    r.register("objectDouble", |b, v: f64| b.object_double = Some(v))
        .unwrap();
    r.register("bigDecimal", |b, v: Decimal| b.big_decimal = Some(v))
        .unwrap();
    r.register("string", |b, v: String| b.string = Some(v))
        .unwrap();
    r.register("configUtilsTestBeanEnum", |b, v: Kind| b.kind = Some(v))
        .unwrap();
    r
}
#[test]
fn merge_property() {
    for (a, b, want) in [
        (None, None, None),
        (Some(1), None, None),
        (None, Some(1), None),
        (Some(1), Some(10), None),
        (Some(1), Some(1), Some(1)),
    ] {
        assert_eq!(merge_config_property(a, b), want);
    }
}
#[test]
fn meld_property() {
    assert_eq!(meld_config_property::<i32>(None, None), None);
    assert_eq!(meld_config_property(Some(1), None), Some(1));
    assert_eq!(meld_config_property(None, Some(10)), Some(10));
    assert_eq!(
        meld_config_property(Some(1), Some(10)),
        merge_config_property(Some(1), Some(10))
    );
}
#[test]
fn ceil_divide() {
    for (a, b, want) in [
        (19, 2, 10),
        (20, 2, 10),
        (21, 2, 11),
        (19, -2, -9),
        (20, -2, -10),
        (21, -2, -10),
        (-19, 2, -9),
        (-20, 2, -10),
        (-21, 2, -10),
        (-19, -2, 10),
        (-20, -2, 10),
        (-21, -2, 11),
    ] {
        assert_eq!(ceil_divide_config(a, b), Ok(want));
    }
}
#[test]
fn ceil_divide_by_zero() {
    assert_eq!(ceil_divide_config(20, 0), Err(ConfigDivisionError::Zero));
}
#[test]
fn apply_custom_properties() {
    let mut b = Bean::default();
    registry()
        .apply(
            &mut b,
            "bean",
            [
                ("primitiveBoolean", "true"),
                ("objectBoolean", "true"),
                ("primitiveInt", "1"),
                ("objectInteger", "2"),
                ("primitiveLong", "3"),
                ("objectLong", "4"),
                ("primitiveFloat", "5.5"),
                ("objectFloat", "6.6"),
                ("primitiveDouble", "7.7"),
                ("objectDouble", "8.8"),
                ("bigDecimal", "9.9"),
                ("string", "This is a sentence."),
                ("configUtilsTestBeanEnum", "BETA"),
            ],
            "customProperties",
        )
        .unwrap();
    assert!(b.primitive_boolean);
    assert_eq!(b.object_boolean, Some(true));
    assert_eq!(b.primitive_int, 1);
    assert_eq!(b.object_integer, Some(2));
    assert_eq!(b.primitive_long, 3);
    assert_eq!(b.object_long, Some(4));
    assert_eq!(f64::from(b.primitive_float), 5.5);
    assert_eq!(b.object_float, Some(6.6_f32));
    assert_eq!(b.primitive_double, 7.7);
    assert_eq!(b.object_double, Some(8.8));
    assert_eq!(b.big_decimal, Some("9.9".parse().unwrap()));
    assert_eq!(b.string.as_deref(), Some("This is a sentence."));
    assert_eq!(b.kind, Some(Kind::Beta));
}
#[test]
fn apply_custom_properties_subset() {
    let mut b = Bean::default();
    registry()
        .apply(
            &mut b,
            "bean",
            [("string", "This is a sentence.")],
            "customProperties",
        )
        .unwrap();
    assert_eq!(b.string.as_deref(), Some("This is a sentence."));
}
#[test]
fn apply_custom_properties_non_existing_custom_property() {
    let mut b = Bean::default();
    assert!(matches!(
        registry().apply(
            &mut b,
            "bean",
            [("doesNotExist", "This is a sentence.")],
            "customProperties"
        ),
        Err(ConfigPropertyError::Unknown { .. })
    ));
}
#[test]
fn abbreviate() {
    assert_eq!(abbreviate_config::<&str>(None::<Vec<&str>>), "");
    assert_eq!(abbreviate_config(Some(Vec::<&str>::new())), "");
    assert_eq!(abbreviate_config(Some(["A", "B", "C"])), "A, B, C");
    assert_eq!(
        abbreviate_config(Some(["A", "B", "C", "D"])),
        "A, B, C, ..."
    );
}
