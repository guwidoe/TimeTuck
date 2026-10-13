// Adapted ConfigUtilsTest constructor helpers; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[derive(Debug)]
pub struct StaticInnerClass;
pub fn static_inner() -> StaticInnerClass {
    StaticInnerClass
}
#[derive(Debug, Eq, PartialEq)]
pub struct StaticInnerClassWithArgsConstructor(pub i32);
pub fn with_args(value: &i32) -> StaticInnerClassWithArgsConstructor {
    StaticInnerClassWithArgsConstructor(*value)
}
pub struct ConfigUtilsTest {
    pub name: &'static str,
}
pub struct NonStaticInnerClass(pub &'static str);
pub fn non_static(owner: &ConfigUtilsTest) -> NonStaticInnerClass {
    NonStaticInnerClass(owner.name)
}
