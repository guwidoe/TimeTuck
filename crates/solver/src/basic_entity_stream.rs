//! Entity rows spanning selected basic planning variables.
use crate::{AdditiveScore, GraphError, PlanningStreamSource, StreamGraphBuilder, StreamPort};
use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BasicEntityRowsError {
    EmptyRow {
        entity: usize,
    },
    UnknownVariable {
        variable: usize,
        variable_count: usize,
    },
    DuplicateVariable {
        variable: usize,
    },
}
impl fmt::Display for BasicEntityRowsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid basic entity rows: {self:?}")
    }
}
impl Error for BasicEntityRowsError {}
/// Static entity-to-variable mapping. Row index is the entity ID; column order is
/// caller-defined and rows may have different widths. Individual rows are nonempty
/// and slots are disjoint and below variable_count. An empty entity set is valid.
/// Unselected variables have no effect. Pinning does not remove an entity's facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BasicEntityRows {
    slots: Arc<BTreeMap<usize, (usize, usize)>>,
    widths: Arc<Vec<usize>>,
}
impl BasicEntityRows {
    pub fn new(variable_count: usize, rows: Vec<Vec<usize>>) -> Result<Self, BasicEntityRowsError> {
        let mut slots = BTreeMap::new();
        let mut widths = vec![];
        for (entity, row) in rows.into_iter().enumerate() {
            if row.is_empty() {
                return Err(BasicEntityRowsError::EmptyRow { entity });
            }
            widths.push(row.len());
            for (column, variable) in row.into_iter().enumerate() {
                if variable >= variable_count {
                    return Err(BasicEntityRowsError::UnknownVariable {
                        variable,
                        variable_count,
                    });
                }
                if slots.insert(variable, (entity, column)).is_some() {
                    return Err(BasicEntityRowsError::DuplicateVariable { variable });
                }
            }
        }
        Ok(Self {
            slots: Arc::new(slots),
            widths: Arc::new(widths),
        })
    }
    pub fn len(&self) -> usize {
        self.widths.len()
    }
    pub fn is_empty(&self) -> bool {
        self.widths.is_empty()
    }
}
pub type BasicEntityStream<V> = StreamPort<usize, (usize, Vec<Option<V>>)>;
/// Project standard planning-variable facts into whole entity rows. A row appears
/// only after all its selected source facts are present. Unless include_unassigned
/// is true, every selected value must also be Some. Null allowance in the domain
/// does not override this filter. Entity IDs and column order remain stable across
/// changes, retractions and reinsertion; duplicate field values remain distinct.
///
/// Use a standard basic planning source or MixedStreamSources::basic, whose tuple
/// ID and payload variable index agree. The schema's variable_count must describe
/// that model; omitted source facts never produce phantom complete entities.
/// Existing graph transactions provide scored-move rollback and cache consistency.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![0,1]).with_initial(0);2])?;
/// let schema=BasicEntityRows::new(2,vec![vec![0,1]])?;
/// let mut b=StreamGraphBuilder::<SimpleScore>::new();
/// let source=b.source();
/// let entities=basic_entity_stream(&mut b,&source,&schema,false)?;
/// b.score("one row per entity",&entities,|_|SimpleScore::new(-1))?;
/// let director=StreamScoreDirector::new(&p,b.build(),source,p.initial_assignment())?;
/// assert_eq!(director.score(),SimpleScore::new(-1));
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn basic_entity_stream<'a, V: Clone + 'static, S: AdditiveScore + 'static>(
    builder: &mut StreamGraphBuilder<'a, S>,
    basics: &PlanningStreamSource<V>,
    schema: &BasicEntityRows,
    include_unassigned: bool,
) -> Result<BasicEntityStream<V>, GraphError> {
    let slots = schema.slots.clone();
    let selected = builder.flat_map(basics.port(), move |(variable, value)| {
        slots
            .get(variable)
            .map(|&(entity, column)| (entity, column, value.clone()))
    })?;
    let grouped = builder.group_list(&selected, |(entity, column, value)| {
        (*entity, (*column, value.clone()))
    })?;
    let widths = schema.widths.clone();
    let visible = builder.filter(&grouped, move |(entity, fields)| {
        fields.len() == widths[*entity]
            && (include_unassigned || fields.iter().all(|(_, value)| value.is_some()))
    })?;
    builder.map(&visible, |(entity, fields)| {
        let mut ordered = vec![None; fields.len()];
        for (column, value) in fields.iter() {
            ordered[*column] = value.clone();
        }
        (*entity, ordered)
    })
}
