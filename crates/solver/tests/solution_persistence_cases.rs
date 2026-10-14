// Complete Community solution-file round-trip TEST adaptations; Apache-2.0.
#[path = "fixtures/solution_persistence_fixture.rs"]
mod fixture;
use timetuck::*;
#[test]
fn jackson_read_and_write() {
    fixture::source_roundtrip(BasicSolutionJsonCodec, "testdataSolution.json", -321)
}
#[test]
fn jaxb_read_and_write() {
    fixture::source_roundtrip(BasicSolutionXmlCodec, "testdataSolution.xml", -123)
}
