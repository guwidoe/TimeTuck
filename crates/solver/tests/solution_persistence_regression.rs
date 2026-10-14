#[path = "fixtures/solution_persistence_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::*;
#[test]
fn decoded_documents_solve_with_both_prepared_backends_then_persist_canonical_values() {
    for stream in [false, true] {
        let directory = Directory::new();
        let json = SolutionFileIo::new(BasicSolutionJsonCodec);
        let xml = SolutionFileIo::new(BasicSolutionXmlCodec);
        let json_path = directory.0.join("problem.json");
        json.write(&original(-321), &json_path).unwrap();
        let decoded: BasicSolutionDocument = json.read(&json_path).unwrap();
        assert_eq!(independent_score(&decoded), SimpleScore::new(1));
        let solved = solve_owned(decoded, stream);
        assert_eq!(solved.score, Some(SimpleScore::new(3)));
        let xml_path = directory.0.join("solved.xml");
        xml.write(&solved, &xml_path).unwrap();
        let copy: BasicSolutionDocument = xml.read(&xml_path).unwrap();
        assert_eq!(copy.score, Some(independent_score(&copy)));
        assert_eq!(copy.score, Some(SimpleScore::new(3)));
        for e in &copy.entities {
            let value = e.value.as_ref().unwrap();
            let canonical = copy.values.iter().find(|v| v.code == value.code).unwrap();
            assert!(Arc::ptr_eq(value, canonical));
        }
        assert!(
            copy.problem()
                .unwrap()
                .is_complete(&copy.problem().unwrap().initial_assignment())
        );
    }
}
#[test]
fn canonical_validation_and_byte_limits_preserve_existing_file_before_publication() {
    let directory = Directory::new();
    let io = SolutionFileIo::new(BasicSolutionJsonCodec);
    let path = directory.0.join("keep.json");
    io.write(&original(-7), &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut invalid = original(-9);
    invalid.entities[1].value = Some(Arc::new(PersistedValue { code: "v1".into() }));
    assert!(
        io.write(&invalid, &path)
            .unwrap_err()
            .to_string()
            .contains("noncanonical")
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let small = SolutionFileIo::new(BasicSolutionJsonCodec)
        .with_byte_limit(10)
        .unwrap();
    assert!(matches!(
        small.write(&original(0), &path),
        Err(SolutionFileError::TooLarge { .. })
    ));
    assert!(matches!(
        small.read::<BasicSolutionDocument>(&path),
        Err(SolutionFileError::TooLarge { .. })
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
    assert!(
        SolutionFileIo::new(BasicSolutionJsonCodec)
            .with_byte_limit(0)
            .is_err()
    );
    assert!(
        SolutionFileIo::new(BasicSolutionJsonCodec)
            .with_byte_limit(usize::MAX)
            .is_err()
    );
    let exact = SolutionFileIo::new(BasicSolutionJsonCodec)
        .with_byte_limit(before.len())
        .unwrap();
    let copy: BasicSolutionDocument = exact.read(&path).unwrap();
    exact.write(&copy, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
#[test]
fn malformed_graphs_scores_and_structures_reject_before_model_publication() {
    let codec = BasicSolutionJsonCodec;
    let good = codec.encode(&original(0)).unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&good).unwrap();
    for mutation in [0, 1, 2, 3, 4] {
        let mut bad = v.clone();
        match mutation {
            0 => bad["valueList"][1]["code"] = "v1".into(),
            1 => bad["entityList"][1]["valueRef"] = "absent".into(),
            2 => bad["entityList"][2]["code"] = "e1".into(),
            3 => bad["score"]["score"] = "invalid".into(),
            _ => bad["unexpected"] = true.into(),
        };
        let result: Result<BasicSolutionDocument, _> =
            codec.decode(&serde_json::to_vec(&bad).unwrap());
        assert!(result.is_err());
    }
    v["score"]["type"] = "hard_soft".into();
    let result: Result<BasicSolutionDocument, _> = codec.decode(&serde_json::to_vec(&v).unwrap());
    assert!(result.is_err());
    let xml = BasicSolutionXmlCodec;
    let good = String::from_utf8(xml.encode(&original(0)).unwrap()).unwrap();
    for bad in [
        good.replace("<valueRef>v1</valueRef>", "<valueRef>absent</valueRef>"),
        good.replace("<version>1</version>", "<version>2</version>"),
        good.replace("<solution>", "<solution unknown=\"true\">"),
        good.replace("<code>s1</code>", "<code>s1</code><code>other</code>"),
        good.replace("<solution>", "<solution xmlns=\"urn:wrong\">"),
        "<!DOCTYPE solution [<!ENTITY x SYSTEM 'file:///unused'>]><solution/>".into(),
    ] {
        let result: Result<BasicSolutionDocument, _> = xml.decode(bad.as_bytes());
        assert!(result.is_err());
    }
}
#[test]
fn varied_labels_nullable_and_structural_scores_roundtrip_in_native_formats() {
    for count in [0, 1, 5] {
        let mut input = original(-123);
        input.code = "& < Unicode λ \" ' \r\n".into();
        input.entities.truncate(count.min(3));
        input.values[0] = Arc::new(PersistedValue {
            code: input.code.clone(),
        });
        if input.entities.len() > 1 {
            input.entities[1].value = Some(input.values[0].clone())
        }
        input.score = Some(SimpleScore::new(-123).with_structural(-2));
        let json = BasicSolutionJsonCodec;
        let xml = BasicSolutionXmlCodec;
        let copies: [BasicSolutionDocument; 2] = [
            json.decode(&json.encode(&input).unwrap()).unwrap(),
            xml.decode(&xml.encode(&input).unwrap()).unwrap(),
        ];
        for copy in copies {
            assert_eq!(copy.code, input.code);
            assert_eq!(copy.values[0].code, input.values[0].code);
            assert_eq!(copy.entities.len(), input.entities.len());
            assert_eq!(copy.score, input.score);
        }
        input.score = None;
        let copy: BasicSolutionDocument = xml.decode(&xml.encode(&input).unwrap()).unwrap();
        assert!(copy.score.is_none());
    }
    let input = BasicSolutionDocument::<HardSoftScore> {
        code: "hard".into(),
        values: vec![],
        entities: vec![],
        score: Some(HardSoftScore::new(-2, 7).with_structural(-1)),
    };
    let copy: BasicSolutionDocument<HardSoftScore> = BasicSolutionXmlCodec
        .decode(&BasicSolutionXmlCodec.encode(&input).unwrap())
        .unwrap();
    assert_eq!(copy.score, input.score);
    let input = BasicSolutionDocument::<BendableScore<2, 1>> {
        code: "bendable".into(),
        values: vec![],
        entities: vec![],
        score: Some(BendableScore::new([-2, 7], [9]).with_structural(-1)),
    };
    let bytes = BasicSolutionXmlCodec.encode(&input).unwrap();
    let copy: BasicSolutionDocument<BendableScore<2, 1>> =
        BasicSolutionXmlCodec.decode(&bytes).unwrap();
    assert_eq!(copy.score, input.score);
    let wrong: Result<BasicSolutionDocument<BendableScore<1, 2>>, _> =
        BasicSolutionXmlCodec.decode(&bytes);
    assert!(wrong.is_err());
}

#[test]
fn invalid_xml_characters_and_assignment_commit_fail_without_mutation() {
    let directory = Directory::new();
    let path = directory.0.join("unchanged.xml");
    let io = SolutionFileIo::new(BasicSolutionXmlCodec);
    let mut input = original(-7);
    io.write(&input, &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    for bad in ['\0', '\u{1f}', '\u{fffe}', '\u{ffff}'] {
        let mut invalid = input.clone();
        invalid.code.push(bad);
        assert!(matches!(
            io.write(&invalid, &path),
            Err(SolutionFileError::Codec(_))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    for assignment in [
        Assignment::new(vec![Some("v2".into())]),
        Assignment::new(vec![Some("v2".into()), Some("missing".into()), None]),
    ] {
        assert!(input.commit(&assignment, SimpleScore::new(100)).is_err());
        assert_eq!(input.score, Some(SimpleScore::new(-7)));
        assert!(input.entities[0].value.is_none());
        assert!(Arc::ptr_eq(
            input.entities[1].value.as_ref().unwrap(),
            &input.values[0]
        ));
    }
    input
        .commit(
            &Assignment::new(vec![Some("v2".into()), Some("v2".into()), None]),
            SimpleScore::new(2),
        )
        .unwrap();
    assert!(Arc::ptr_eq(
        input.entities[0].value.as_ref().unwrap(),
        &input.values[1]
    ));
    assert!(Arc::ptr_eq(
        input.entities[1].value.as_ref().unwrap(),
        &input.values[1]
    ));
    assert_eq!(input.score, Some(independent_score(&input)));
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
}
#[test]
fn executable_custom_codec_and_failed_rename_cleanup_are_real_file_operations() {
    struct Number;
    impl SolutionCodec<i64> for Number {
        fn encode(&self, n: &i64) -> Result<Vec<u8>, String> {
            Ok(n.to_string().into_bytes())
        }
        fn decode(&self, b: &[u8]) -> Result<i64, String> {
            std::str::from_utf8(b)
                .map_err(|e| e.to_string())?
                .parse::<i64>()
                .map_err(|e| e.to_string())
        }
    }
    let directory = Directory::new();
    let path = directory.0.join("value");
    let io = SolutionFileIo::new(Number);
    io.write(&-91, &path).unwrap();
    assert_eq!(io.read::<i64>(&path).unwrap(), -91);
    let blocked = directory.0.join("directory");
    std::fs::create_dir(&blocked).unwrap();
    assert!(io.write(&7, &blocked).is_err());
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 2);
    std::fs::write(&path, b"not a number").unwrap();
    assert!(io.read::<i64>(&path).is_err());
}
