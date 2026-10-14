// Full adapted Community AbstractTrivialModelConvertorTest methods; Apache-2.0.
#[path = "fixtures/trivial_model_convertor_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::*;
#[test]
fn to_solver_model_rejects_null_arguments() {
    let (convertor, model, config) = setup();
    for (input, config, previous, expected) in [
        (
            None,
            Some(config.clone()),
            Some(PreviousModelOutput::Empty),
            "modelInput",
        ),
        (
            Some(model.clone()),
            None,
            Some(PreviousModelOutput::Empty),
            "modelConfig",
        ),
        (Some(model), Some(config), None, "lastModelOutput"),
    ] {
        let request = ModelConversionRequest {
            model_input: input,
            model_config: config,
            last_model_output: previous,
        };
        assert_eq!(
            convertor.to_solver_model(&request).unwrap_err().to_string(),
            expected
        );
    }
}
#[test]
fn to_model_output_rejects_null_solver_model() {
    let (convertor, _, _) = setup();
    assert_eq!(
        convertor.to_model_output(None).unwrap_err().to_string(),
        "solverModel"
    );
}
#[test]
fn apply_output_to_input_rejects_null_arguments() {
    let (convertor, model, _) = setup();
    assert_eq!(
        convertor
            .apply_output_to_input(None, Some(&model))
            .unwrap_err()
            .to_string(),
        "modelInput"
    );
    assert_eq!(
        convertor
            .apply_output_to_input(Some(&model), None)
            .unwrap_err()
            .to_string(),
        "modelOutput"
    );
}
#[test]
fn apply_output_to_input_returns_model_output_when_types_match() {
    let (convertor, model, _) = setup();
    let output = Arc::new(TestModel::default());
    let replacement = convertor
        .apply_output_to_input(Some(&model), Some(&output))
        .unwrap();
    assert!(Arc::ptr_eq(&replacement, &output));
}
#[test]
fn to_solver_model_uses_empty_optional_when_no_previous_output() {
    let (convertor, model, config) = setup();
    let request = ModelConversionRequest::new(model.clone(), config, None);
    let solver = convertor.to_solver_model(&request).unwrap();
    assert!(Arc::ptr_eq(&solver, &model));
    assert!(solver.score.is_none() && solver.constraint_weight_overrides.is_none());
}
