// Adapted pinned Community TEST helper fields/getters/updateValue; Apache-2.0.
#[derive(Debug, Eq, PartialEq)]
struct TestValue(String);
struct FieldAnnotatedEntity {value: Arc<TestValue>}
impl FieldAnnotatedEntity {
    fn get_value(&self)->Arc<TestValue> {self.value.clone()}
    fn set_value(&mut self,value:Arc<TestValue>) {self.value=value;}
}
struct GizmoEntity {id:String,value:Arc<TestValue>,generic:Vec<std::collections::BTreeMap<String,String>>}
impl GizmoEntity {
    fn get_id(&self)->String {self.id.clone()}
    fn get_value(&self)->Arc<TestValue> {self.value.clone()}
    fn set_value(&mut self,value:Arc<TestValue>) {self.value=value;}
    fn read_method(&self)->String {"Read Method".into()}
}
struct VisibilitySolution {private_property:String}
struct MutableEntity {value:Arc<TestValue>}
impl MutableEntity {
    fn update_value(&mut self) {self.value=Arc::new(TestValue(format!("{0}/{0}",self.value.0)));}
}
fn field()->ExecutableMemberDescriptor<GizmoEntity,Arc<TestValue>> {ExecutableMemberDescriptor::field("value",GizmoEntity::get_value,GizmoEntity::set_value).unwrap()}
fn id()->ExecutableMemberDescriptor<GizmoEntity,String> {ExecutableMemberDescriptor::method("getId",MemberExposure::Public,GizmoEntity::get_id).unwrap()}
fn read_method()->ExecutableMemberDescriptor<GizmoEntity,String> {ExecutableMemberDescriptor::method("readMethod",MemberExposure::Public,GizmoEntity::read_method).unwrap()}
