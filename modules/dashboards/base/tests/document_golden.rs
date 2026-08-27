use mfa_contracts::{
    AvailabilityState, CapabilityId, DashboardBlock, DashboardDocument, DashboardInput,
};
use mfa_dashboard_base::{BasePage, compose_json, compose_page, describe_module};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn capability(name: &str) -> CapabilityId {
    CapabilityId::try_from(name).unwrap()
}

fn ready_input() -> DashboardInput {
    DashboardInput {
        page_id: None,
        capabilities: BTreeMap::from([
            (
                capability("body.weight"),
                json!([
                    {"date": "2026-01-01", "value": 82.5},
                    {"date": "2026-01-02", "value": 82.3}
                ]),
            ),
            (capability("body.fat_percentage"), json!({"value": 18.0})),
            (
                capability("nutrition.items"),
                json!({"calories": 2_400, "complete_days": 28}),
            ),
            (
                capability("activity.days"),
                json!({"steps": 9_000, "accepted_events": 2}),
            ),
            (capability("strength.sessions"), json!({"sessions": 4})),
            (capability("strength.sets"), json!({"sets": 24})),
        ]),
        extensions: BTreeMap::new(),
    }
}

fn empty_input() -> DashboardInput {
    DashboardInput {
        page_id: None,
        capabilities: BTreeMap::new(),
        extensions: BTreeMap::new(),
    }
}

fn block_keys(document: &DashboardDocument) -> Vec<String> {
    document
        .blocks
        .iter()
        .map(|block| match block {
            DashboardBlock::Card(card) => card.key.clone(),
            DashboardBlock::Table(table) => table.key.clone(),
            DashboardBlock::StatusPanel(panel) => panel.key.clone(),
            DashboardBlock::Chart(chart) => chart.key.clone(),
        })
        .collect()
}

fn assert_golden(document: &DashboardDocument, filename: &str) {
    let path = format!("{}/tests/golden/{filename}", env!("CARGO_MANIFEST_DIR"));
    let expected: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(document).unwrap(), expected);
}

#[test]
fn every_required_page_has_reviewed_ready_document_shape() {
    let cases = [
        (
            BasePage::Overview,
            "base.overview.title",
            "overview.body_weight",
            "overview-ready.json",
        ),
        (
            BasePage::Body,
            "base.body.title",
            "body.weight.trend",
            "body-ready.json",
        ),
        (
            BasePage::Nutrition,
            "base.nutrition.title",
            "nutrition.calories",
            "nutrition-ready.json",
        ),
        (
            BasePage::Activity,
            "base.activity.title",
            "activity.steps",
            "activity-ready.json",
        ),
        (
            BasePage::Strength,
            "base.strength.title",
            "strength.sessions",
            "strength-ready.json",
        ),
        (
            BasePage::Sources,
            "base.sources.title",
            "sources.modules",
            "sources-ready.json",
        ),
    ];
    for (page, title_key, required_key, filename) in cases {
        let document = compose_page(page, &ready_input());
        assert_eq!(document.title_key, title_key);
        assert!(block_keys(&document).contains(&required_key.to_owned()));
        assert!(document.is_declarative());
        assert_golden(&document, filename);
    }
}

#[test]
fn missing_page_inputs_keep_graphs_visible_with_exact_state() {
    let cases = [
        (BasePage::Overview, "overview-empty.json"),
        (BasePage::Body, "body-empty.json"),
        (BasePage::Nutrition, "nutrition-empty.json"),
        (BasePage::Activity, "activity-empty.json"),
        (BasePage::Strength, "strength-empty.json"),
        (BasePage::Sources, "sources-empty.json"),
    ];
    for (page, filename) in cases {
        let document = compose_page(page, &empty_input());
        assert!(document.blocks.iter().any(|block| matches!(
            block,
            DashboardBlock::StatusPanel(panel) if panel.state == AvailabilityState::MissingCapability
        )));
        if page != BasePage::Sources {
            assert!(document.blocks.iter().any(|block| matches!(block, DashboardBlock::Chart(chart) if chart.series.iter().all(|series| series.points.is_empty()))));
        }
        assert_golden(&document, filename);
    }
}

#[test]
fn wit_adapter_describes_base_and_selects_requested_page() {
    let description: serde_json::Value = serde_json::from_str(&describe_module()).unwrap();
    assert_eq!(description["module_id"], "base");
    assert_eq!(description["localization_namespace"], "base");
    let input = serde_json::to_value(ready_input()).unwrap();
    let mut input = input.as_object().unwrap().clone();
    input.insert("page_id".to_owned(), json!("body"));
    let document: DashboardDocument =
        serde_json::from_str(&compose_json(&serde_json::to_string(&input).unwrap()).unwrap())
            .unwrap();
    assert_eq!(document.title_key, "base.body.title");
    assert_golden(&document, "body-ready.json");
}

#[test]
fn chart_extracts_object_dataset_points_and_preserves_gaps() {
    let input = DashboardInput {
        page_id: None,
        capabilities: BTreeMap::from([(
            capability("body.weight"),
            json!({
                "trailing7dMeanKg": [
                    {"local_date": "2026-01-01", "value": 82.5},
                    {"local_date": "2026-01-02", "value": null},
                    {"local_date": "2026-01-03", "value": 82.2}
                ]
            }),
        )]),
        extensions: BTreeMap::new(),
    };

    let document = compose_page(BasePage::Body, &input);
    let chart = document
        .blocks
        .iter()
        .find_map(|block| match block {
            DashboardBlock::Chart(chart) => Some(chart),
            _ => None,
        })
        .expect("body page chart");

    assert_eq!(
        serde_json::to_value(&chart.series[0].points).unwrap(),
        json!([
            ["2026-01-01", 82.5],
            ["2026-01-02", null],
            ["2026-01-03", 82.2]
        ])
    );
}

#[test]
fn activity_cards_use_their_declared_capabilities_independently() {
    let input = DashboardInput {
        page_id: Some("activity".to_owned()),
        capabilities: BTreeMap::from([
            (
                capability("activity.days"),
                json!({"steps": [{"local_date": "2026-01-01", "value": 9000}]}),
            ),
            (
                capability("activity.events"),
                json!({"events": [{"local_date": "2026-01-01", "accepted_event_count": 2}]}),
            ),
            (
                capability("heart_rate.observations"),
                json!({"observations": []}),
            ),
        ]),
        extensions: BTreeMap::new(),
    };

    let document = compose_page(BasePage::Activity, &input);
    let events = document
        .blocks
        .iter()
        .find_map(|block| match block {
            DashboardBlock::Card(card) if card.key == "activity.events" => Some(&card.value),
            _ => None,
        })
        .expect("activity events card");
    assert_eq!(events["value"]["events"][0]["accepted_event_count"], 2);
}

#[test]
fn body_page_renders_optional_body_fat_series_without_weight_substitution() {
    let input = DashboardInput {
        page_id: Some("body".to_owned()),
        capabilities: BTreeMap::from([
            (
                capability("body.weight"),
                json!({"observations": [{"local_date": "2026-01-01", "value_kg": 82.0}]}),
            ),
            (
                capability("body.fat_percentage"),
                json!({"observations": [
                    {"local_date": "2026-01-01", "value": 18.5},
                    {"local_date": "2026-01-02", "value": null}
                ]}),
            ),
        ]),
        extensions: BTreeMap::new(),
    };

    let document = compose_page(BasePage::Body, &input);
    let chart = document
        .blocks
        .iter()
        .find_map(|block| match block {
            DashboardBlock::Chart(chart) if chart.key == "body.fat.trend" => Some(chart),
            _ => None,
        })
        .expect("body fat chart");
    assert_eq!(chart.series[0].points[0].1, Some(18.5));
    assert_eq!(chart.series[0].points[1].1, None);
}
