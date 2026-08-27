use mfa_contracts::{
    AvailabilityState, CapabilityId, DashboardBlock, DashboardCard, DashboardChart,
    DashboardDocument, DashboardInput, DashboardSeries, DashboardStatusPanel, DashboardTable,
};
use mfa_dashboard_host::{
    DocumentValidationError, validate_document, validate_document_json, validate_or_error_result,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn grant() -> DashboardInput {
    DashboardInput {
        page_id: None,
        capabilities: BTreeMap::from([(
            CapabilityId::try_from("body.weight").unwrap(),
            json!({"kg": 82.5}),
        )]),
        extensions: BTreeMap::from([("workouts.rpe".to_owned(), json!({"rpe": 8}))]),
    }
}

fn keys() -> BTreeSet<String> {
    [
        "dashboard.title",
        "dashboard.weight",
        "dashboard.status",
        "dashboard.series",
        "dashboard.column",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn safe_document() -> DashboardDocument {
    DashboardDocument {
        title_key: "dashboard.title".to_owned(),
        blocks: vec![
            DashboardBlock::Card(DashboardCard {
                key: "weight-card".to_owned(),
                label: "dashboard.weight".to_owned(),
                value: json!({"dataset": "body.weight", "value": 82.5}),
            }),
            DashboardBlock::Chart(DashboardChart {
                key: "weight-chart".to_owned(),
                chart_type: "line".to_owned(),
                series: vec![DashboardSeries {
                    name: "dashboard.series".to_owned(),
                    points: vec![("2026-01-01".to_owned(), Some(82.5))],
                }],
            }),
            DashboardBlock::StatusPanel(DashboardStatusPanel {
                key: "status".to_owned(),
                state: AvailabilityState::Ready,
                message_key: "dashboard.status".to_owned(),
            }),
            DashboardBlock::Table(DashboardTable {
                key: "table".to_owned(),
                columns: vec!["dashboard.column".to_owned()],
                rows: vec![vec![json!(82.5)]],
            }),
        ],
    }
}

#[test]
fn validator_accepts_only_supported_safe_document_primitives() {
    validate_document(&safe_document(), &grant(), &keys()).unwrap();
}

#[test]
fn validator_rejects_unknown_types_and_active_content() {
    let cases = [
        (
            r#"{"title_key":"dashboard.title","blocks":[{"type":"html","value":"<div>bad</div>"}]}"#,
            "unknown_node_type",
        ),
        (
            r#"{"title_key":"dashboard.title","blocks":[{"type":"chart","value":{"key":"x","chart_type":"line","series":[{"name":"dashboard.series","points":[["javascript:alert(1)",1.0]]}]}}]}"#,
            "unsafe_string",
        ),
        (
            r#"{"title_key":"dashboard.title","blocks":[{"type":"card","value":{"key":"x","label":"dashboard.weight","value":{"onClick":"bad"}}}]}"#,
            "forbidden_key",
        ),
        (
            r#"{"title_key":"dashboard.title","blocks":[{"type":"card","value":{"key":"x","label":"dashboard.weight","value":{"css":"color:red"}}}]}"#,
            "forbidden_key",
        ),
        (
            r#"{"title_key":"dashboard.title","blocks":[{"type":"card","value":{"key":"x","label":"dashboard.weight","value":{"url":"https://example.invalid"}}}]}"#,
            "forbidden_key",
        ),
        (
            r#"{"title_key":"dashboard.title","blocks":[{"type":"card","value":{"key":"x","label":"dashboard.weight","value":{"sql":"SELECT * FROM secrets"}}}]}"#,
            "forbidden_key",
        ),
    ];
    for (raw, code) in cases {
        let error = validate_document_json(&serde_json::from_str(raw).unwrap(), &grant(), &keys())
            .unwrap_err();
        assert_eq!(error.code(), code, "case {raw}");
    }
}

#[test]
fn validator_rejects_nonfinite_oversized_undeclared_and_unsafe_chart_values() {
    let mut document = safe_document();
    if let DashboardBlock::Chart(chart) = &mut document.blocks[1] {
        chart.chart_type = "radar".to_owned();
    }
    assert_eq!(
        validate_document(&document, &grant(), &keys())
            .unwrap_err()
            .code(),
        "unknown_chart_type"
    );

    let mut document = safe_document();
    if let DashboardBlock::Chart(chart) = &mut document.blocks[1] {
        chart.chart_type = "scatter".to_owned();
        chart.series[0].points[0].1 = Some(f64::NAN);
    }
    assert_eq!(
        validate_document(&document, &grant(), &keys())
            .unwrap_err()
            .code(),
        "non_finite_number"
    );

    let mut document = safe_document();
    if let DashboardBlock::Card(card) = &mut document.blocks[0] {
        card.value = json!({"dataset": "private.secret"});
    }
    assert_eq!(
        validate_document(&document, &grant(), &keys())
            .unwrap_err()
            .code(),
        "undeclared_dataset"
    );

    let mut document = safe_document();
    document.blocks[0] = DashboardBlock::Chart(DashboardChart {
        key: "huge".to_owned(),
        chart_type: "line".to_owned(),
        series: vec![DashboardSeries {
            name: "dashboard.series".to_owned(),
            points: (0..10_001)
                .map(|index| (index.to_string(), Some(index as f64)))
                .collect(),
        }],
    });
    assert_eq!(
        validate_document(&document, &grant(), &keys())
            .unwrap_err()
            .code(),
        "series_too_large"
    );

    let mut document = safe_document();
    document.title_key = "undeclared.title".to_owned();
    let error = validate_document(&document, &grant(), &keys()).unwrap_err();
    assert!(matches!(
        error,
        DocumentValidationError::UndeclaredLocalization { .. }
    ));
}

#[test]
fn validator_rejects_unknown_fields_at_every_document_shape() {
    let cases = [
        r#"{"title_key":"dashboard.title","blocks":[],"onClick":"bad"}"#,
        r#"{"title_key":"dashboard.title","blocks":[{"type":"card","value":{"key":"x","label":"dashboard.weight","value":82.5},"onClick":"bad"}]}"#,
        r#"{"title_key":"dashboard.title","blocks":[{"type":"card","value":{"key":"x","label":"dashboard.weight","value":82.5,"onClick":"bad"}}]}"#,
        r#"{"title_key":"dashboard.title","blocks":[{"type":"table","value":{"key":"x","columns":[],"rows":[],"onClick":"bad"}}]}"#,
        r#"{"title_key":"dashboard.title","blocks":[{"type":"status_panel","value":{"key":"x","state":{"type":"ready"},"message_key":"dashboard.status","onClick":"bad"}}]}"#,
        r#"{"title_key":"dashboard.title","blocks":[{"type":"chart","value":{"key":"x","chart_type":"line","series":[],"onClick":"bad"}}]}"#,
        r#"{"title_key":"dashboard.title","blocks":[{"type":"chart","value":{"key":"x","chart_type":"line","series":[{"name":"dashboard.series","points":[],"onClick":"bad"}]}}]}"#,
    ];
    for raw in cases {
        assert_eq!(
            validate_document_json(&serde_json::from_str(raw).unwrap(), &grant(), &keys())
                .unwrap_err()
                .code(),
            "malformed_document",
            "unknown document field was accepted: {raw}"
        );
    }
}

#[test]
fn invalid_guest_output_becomes_a_typed_safe_module_error() {
    let output = validate_or_error_result::<String>(
        Err("guest_invalid_output".to_owned()),
        &grant(),
        &keys(),
    );

    assert_eq!(
        output,
        mfa_dashboard_host::DashboardOutput::ModuleError(mfa_dashboard_host::ModuleErrorView {
            code: "guest_invalid_output".to_owned(),
            message_key: "dashboard.module_error.guest_invalid_output".to_owned(),
        })
    );
}
