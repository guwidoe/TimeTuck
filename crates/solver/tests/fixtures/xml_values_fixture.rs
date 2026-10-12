// Adapted Community GenericJaxbIOTest.DummyJaxbClass TEST helper.
// Apache-2.0; source and modifications in compat/provenance.json.
use timetuck::*;
#[derive(Debug)]
pub struct Dummy {
    pub id: i32,
    pub value: String,
}
impl PartialEq for Dummy {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Dummy {}
pub fn codec() -> XmlObjectCodec<Dummy> {
    XmlObjectCodec::new(
        "dummyJaxbClass",
        None,
        |n| {
            Ok(Dummy {
                id: n
                    .attribute("id")
                    .unwrap_or("0")
                    .parse()
                    .map_err(|_| XmlValueError("invalid id".into()))?,
                value: n.text().unwrap_or("").into(),
            })
        },
        |v| SolverXmlElement {
            name: "dummyJaxbClass".into(),
            attributes: vec![("id".into(), v.id.to_string())],
            text: v.value.clone(),
            children: vec![],
        },
    )
}
