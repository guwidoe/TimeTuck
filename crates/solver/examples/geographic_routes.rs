//! Route planning using generated spherical costs and departure-dependent speeds.
use timetuck::*;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let coordinates = [
        (48.20, 16.37),
        (48.25, 16.37),
        (48.21, 16.38),
        (48.22, 16.39),
        (48.23, 16.36),
        (48.24, 16.35),
    ];
    let places: Vec<_> = coordinates
        .into_iter()
        .map(|(a, b)| GeoLocation::new(a, b))
        .collect::<Result<_, _>>()?;
    let fast = GreatCircleRouting::new(15000)?;
    let slow = GreatCircleRouting::new(5000)?;
    let morning = fast.matrices(&places)?;
    let afternoon = slow.matrices(&places)?;
    let mut travel = TimeframeMatrices::new();
    travel.set_timeframes(
        vec![
            Some(&morning.travel_seconds),
            Some(&afternoon.travel_seconds),
            Some(&morning.travel_seconds),
        ],
        |t: &OffsetDeparture| Daypart::at(*t).index(),
    );
    let departure = 12 * 3600 - 120;
    let p = ListProblem::new(
        places[2..].to_vec(),
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )?;
    let score = |a: &ListAssignment<GeoLocation>| {
        let mut total = 0;
        let mut hard = 0;
        for (vehicle, route) in a.lists().iter().enumerate() {
            hard -= route.len().saturating_sub(2) as i64;
            let mut now = departure;
            let mut from = places[vehicle];
            for &to in route.iter().chain(std::iter::once(&places[vehicle])) {
                let t = OffsetDeparture::new(now, 0).unwrap();
                now += travel.get(&from, &to, Some(&t)).unwrap();
                from = to;
            }
            total += now - departure;
        }
        HardSoftScore::new(hard, -total)
    };
    let result = exhaustive_list_search(&p, score, 1000);
    assert!(result.is_optimal());
    assert_eq!(result.evaluations, 120);
    let best = result.best.unwrap();
    assert!(p.is_complete(&best.assignment));
    let mut oracle = 0;
    for (vehicle, route) in best.assignment.lists().iter().enumerate() {
        assert!(route.len() <= 2);
        let mut now = departure;
        let mut from = places[vehicle];
        for &to in route.iter().chain(std::iter::once(&places[vehicle])) {
            let provider = if now % 86400 >= 43200 && now % 86400 < 64800 {
                slow
            } else {
                fast
            };
            now += provider.estimate(from, to).travel_seconds;
            from = to;
        }
        oracle += now - departure;
    }
    assert_eq!(best.score, HardSoftScore::new(0, -oracle));
    println!(
        "{} · {} assignments · departure-dependent spherical routing",
        best.score, result.evaluations
    );
    Ok(())
}
