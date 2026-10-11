// Pinned Community TEST domains from mixed/singleentity and mixed/multientity.
// Apache-2.0; see compat/provenance.json. Weighted scoring is original.
#![allow(dead_code)]
use std::{cell::Cell, io, rc::Rc};
use timetuck::*;
pub type Model = MixedProblem<String, String>;
pub struct TestdataMixedEntity;
pub struct TestdataMixedValue;
pub struct TestdataMixedOtherValue;
pub struct TestdataMixedMultiEntityFirstEntity;
pub struct TestdataMixedMultiEntitySecondEntity;
pub struct Domain {
    pub model: Rc<Model>,
    pub schema: Rc<PlanningSchema>,
    pub network: MixedConstraintNetwork<'static, String, String, SimpleScore>,
    pub preparations: Rc<Cell<usize>>,
}
impl Domain {
    pub fn new(multiple_entities: bool, weighted: bool) -> Self {
        let basic = FiniteProblem::new(
            (0..4)
                .map(|_| {
                    Variable::new(vec![
                        "Generated Other Value 0".into(),
                        "Generated Other Value 1".into(),
                    ])
                })
                .collect(),
        )
        .unwrap();
        let lists = ListProblem::new(
            vec!["Generated Value 0".into(), "Generated Value 1".into()],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap();
        let model = Rc::new(Model::new(basic, lists));
        let basic_fields = || {
            vec![
                PlanningVariableDefinition::basic::<String>("basicValue", false),
                PlanningVariableDefinition::basic::<String>("secondBasicValue", false),
            ]
        };
        let list_field = || PlanningVariableDefinition::list::<String>("valueList", false);
        let definitions = if multiple_entities {
            vec![
                PlanningEntityDefinition::of::<TestdataMixedMultiEntityFirstEntity>(vec![
                    list_field(),
                ]),
                PlanningEntityDefinition::of::<TestdataMixedMultiEntitySecondEntity>(basic_fields()),
            ]
        } else {
            let mut fields = basic_fields();
            fields.push(list_field());
            fields.push(PlanningVariableDefinition::shadow::<i32>(
                "declarativeShadowVariableValue",
            ));
            vec![
                PlanningEntityDefinition::of::<TestdataMixedEntity>(fields),
                PlanningEntityDefinition::of::<TestdataMixedValue>(vec![
                    PlanningVariableDefinition::shadow::<TestdataMixedEntity>("entity"),
                    PlanningVariableDefinition::shadow::<TestdataMixedValue>("previousElement"),
                    PlanningVariableDefinition::shadow::<TestdataMixedValue>("nextElement"),
                    PlanningVariableDefinition::shadow::<usize>("index"),
                    PlanningVariableDefinition::shadow::<i32>("cascadingShadowVariableValue"),
                    PlanningVariableDefinition::shadow::<i32>("declarativeShadowVariableValue"),
                ]),
                PlanningEntityDefinition::of::<TestdataMixedOtherValue>(vec![
                    PlanningVariableDefinition::shadow::<Vec<TestdataMixedEntity>>("entityList"),
                    PlanningVariableDefinition::shadow::<i32>("declarativeShadowVariableValue"),
                ]),
            ]
        };
        let schema = Rc::new(PlanningSchema::new::<Model>(definitions).unwrap());
        let mut network = MixedConstraintNetwork::<String, String, SimpleScore>::new(4, 2, 2);
        if weighted {
            for i in 0..4 {
                network
                    .add_constraint(
                        format!("basic {i}"),
                        vec![MixedScope::Basic(i)],
                        move |facts| {
                            let MixedFact::Basic(value) = &facts[0] else {
                                unreachable!()
                            };
                            SimpleScore::new(if value.is_some_and(|v| v.ends_with('1')) {
                                i as i64 + 1
                            } else {
                                0
                            })
                        },
                    )
                    .unwrap();
            }
            network
                .add_constraint(
                    "list destination",
                    vec![MixedScope::ListEntity(1)],
                    |facts| {
                        let MixedFact::ListEntity(values) = &facts[0] else {
                            unreachable!()
                        };
                        SimpleScore::new(values.len() as i64 * 10)
                    },
                )
                .unwrap();
        }
        Self {
            model,
            schema,
            network,
            preparations: Rc::new(Cell::new(0)),
        }
    }
    pub fn configuration(
        &self,
        variable: Option<&str>,
        phase: usize,
    ) -> SolverAssemblyConfiguration<'_, Model, SimpleScore> {
        let mut c = SolverAssemblyConfiguration::for_factory_model(self.model.clone());
        c.schema = Some(self.schema.clone());
        c.phases = vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
            minimum_ruined: 1,
            maximum_ruined: 2,
            max_attempts: 6,
            assert_scores: true,
            ..Default::default()
        })];
        c.schema_ruin_phases = vec![SchemaRuinPhaseConfiguration {
            phase,
            selector: SchemaRuinConfig {
                entity: None,
                variable: variable.map(str::to_owned),
            },
            bindings: vec![
                SchemaRuinBinding {
                    entity: PlanningType::of::<TestdataMixedEntity>(),
                    variable: "basicValue".into(),
                    slots: vec![0, 1],
                },
                SchemaRuinBinding {
                    entity: PlanningType::of::<TestdataMixedEntity>(),
                    variable: "secondBasicValue".into(),
                    slots: vec![2, 3],
                },
                SchemaRuinBinding {
                    entity: PlanningType::of::<TestdataMixedEntity>(),
                    variable: "valueList".into(),
                    slots: vec![0, 1],
                },
            ],
        }];
        c
    }
    pub fn declaration<'p>(
        &'p self,
    ) -> ScoringFactoryDeclaration<
        'p,
        &'p Self,
        MixedAssignment<String, String>,
        Resource<'p>,
        io::Error,
    > {
        let prepare = move |_| {
            self.preparations.set(self.preparations.get() + 1);
            Ok(self)
        };
        let build = |domain: &&'p Domain, _settings, input| {
            Ok(Resource {
                domain,
                director: MixedScoreDirector::new(&domain.model, &domain.network, input)
                    .map_err(|e| io::Error::other(e.to_string()))?,
            })
        };
        ScoringFactoryDeclaration::default().with_scoring_recipe(
            ScoringFactoryRecipe::full_evaluation_recipe(prepare, build, |r: &Resource<'_>| {
                Ok(r.director.assignment().clone())
            }),
        )
    }
    pub fn initialized(&self) -> MixedAssignment<String, String> {
        MixedAssignment::new(
            Assignment::new(vec![
                Some(
                    self.model.basic().variables()[0].domain()[0].clone()
                );
                4
            ]),
            ListAssignment::new(vec![self.model.lists().values().to_vec(), vec![]]),
        )
    }
}
pub struct Resource<'p> {
    pub domain: &'p Domain,
    pub director: MixedScoreDirector<'p, 'static, String, String, SimpleScore>,
}
impl ContextScoreResource for Resource<'_> {
    type Score = SimpleScore;
    type Error = io::Error;
    fn context_totals(&self) -> Vec<(String, SimpleScore)> {
        self.director
            .constraint_scores()
            .map(|(n, s)| (n.into(), s))
            .collect()
    }
    fn context_score(&self) -> SimpleScore {
        self.director.score()
    }
    fn check_context(&mut self) -> Result<(), io::Error> {
        self.director
            .assert_consistent()
            .map_err(|e| io::Error::other(e.to_string()))
    }
}
pub struct Work(pub SolverResult<MixedAssignment<String, String>, SimpleScore>);
impl ContextScoreWork for Work {
    fn context_calculations(&self) -> u64 {
        self.0.work.evaluations
    }
}
pub fn shape() -> Rc<FactoryScoreShape> {
    Rc::new(FactoryScoreShape::for_score::<SimpleScore>())
}
