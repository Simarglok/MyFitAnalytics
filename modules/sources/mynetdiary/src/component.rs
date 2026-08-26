wit_bindgen::generate!({
    path: "../../sdk/wit/source-api.wit",
    world: "source-module",
});

use mfa_contracts::{MappingIssue, SourceValidation};

struct Component;

export!(Component);

impl Guest for Component {
    fn metadata() -> String {
        r#"{"module_id":"mynetdiary","module_version":"1.0.0","source_api_version":"1.0.0","mapping_version":"1.0.0","provided_capabilities":["nutrition.items","activity.events"],"extension_contracts":[{"namespace":"mynetdiary.water-glasses","contract_version":"1.0.0"}],"localization_namespace":"source.mynetdiary"}"#.to_owned()
    }

    fn contract_version() -> String {
        "1.0.0".to_owned()
    }

    fn detect(asset: &AssetReader) -> u8 {
        read_asset(asset)
            .map(|bytes| crate::detect_mynetdiary(&bytes))
            .unwrap_or(0)
    }

    fn validate(asset: &AssetReader) -> Result<String, String> {
        let metadata = asset.metadata();
        let bytes = read_asset(asset)?;
        match crate::validate_workbook(&bytes) {
            Ok(schema) => {
                let context = crate::MappingContext::for_workbook(metadata.asset_id, &schema);
                serde_json::to_string(&SourceValidation {
                    valid: true,
                    issues: Vec::new(),
                    source_module_id: context.module_id,
                    source_api_version: "1.0.0"
                        .parse::<mfa_contracts::ContractVersion>()
                        .map_err(|error| error.to_string())?,
                    logical_snapshot_key: context.logical_snapshot_key,
                    schema_fingerprint: context.schema_fingerprint,
                    mapping_version: context.mapping_version,
                })
                .map_err(|error| error.to_string())
            }
            Err(error) => {
                let issue = MappingIssue {
                    code: error.code().to_owned(),
                    message: error.detail(),
                    source_record_key: None,
                };
                serde_json::to_string(&SourceValidation {
                    valid: false,
                    issues: vec![issue],
                    source_module_id: "mynetdiary".to_owned(),
                    source_api_version: "1.0.0"
                        .parse::<mfa_contracts::ContractVersion>()
                        .map_err(|error| error.to_string())?,
                    logical_snapshot_key: "mynetdiary:invalid".to_owned(),
                    schema_fingerprint:
                        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                            .to_owned(),
                    mapping_version: "1.0.0"
                        .parse::<mfa_contracts::ContractVersion>()
                        .map_err(|error| error.to_string())?,
                })
                .map_err(|error| error.to_string())
            }
        }
    }

    fn parse(asset: &AssetReader) -> Result<String, String> {
        let metadata = asset.metadata();
        let bytes = read_asset(asset)?;
        let batch = crate::parse_workbook(&bytes, metadata.asset_id)
            .map_err(|error| format!("{}: {}", error.code(), error.detail()))?;
        serde_json::to_string(&batch).map_err(|error| error.to_string())
    }
}

fn read_asset(asset: &AssetReader) -> Result<Vec<u8>, String> {
    let metadata = asset.metadata();
    let byte_len = u32::try_from(metadata.byte_len)
        .map_err(|_| "asset exceeds guest reader u32 limit".to_owned())?;
    asset.read_at(0, byte_len)
}
