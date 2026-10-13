// Adapted pinned Community field/value/visibility helper bodies; Apache-2.0.
#[derive(Debug, Eq, PartialEq)]
struct Value(String);
struct FieldEntity {
    value: Arc<Value>,
}
impl FieldEntity {
    fn get_value(&self) -> Arc<Value> {
        self.value.clone()
    }
    fn set_value(&mut self, v: Arc<Value>) {
        self.value = v;
    }
}
const CLASS: &str =
    "ai.timefold.solver.core.testdomain.reflect.accessmodifier.TestdataVisibilityModifierSolution";
struct VisibilitySolution {
    private_field: String,
    public_field: String,
    _private_property: String,
    _friendly_property: String,
    _protected_property: String,
    public_property: String,
}
impl VisibilitySolution {
    fn new(values: [&str; 6]) -> Self {
        Self {
            private_field: values[0].into(),
            public_field: values[1].into(),
            _private_property: values[2].into(),
            _friendly_property: values[3].into(),
            _protected_property: values[4].into(),
            public_property: values[5].into(),
        }
    }
    fn get_public_property(&self) -> String {
        self.public_property.clone()
    }
    fn set_public_property(&mut self, v: String) {
        self.public_property = v;
    }
}
fn property() -> MemberFactoryRegistration<VisibilitySolution, String> {
    MemberFactoryRegistration::getter(
        CLASS,
        "getPublicProperty",
        VisibilitySolution::get_public_property,
        VisibilitySolution::set_public_property,
    )
}
