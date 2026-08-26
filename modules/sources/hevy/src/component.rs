wit_bindgen::generate!({
    path: "../../sdk/wit/source-api.wit",
    world: "source-module",
});

use mfa_contracts::{MappingIssue, SourceValidation};

struct Component;

export!(Component);

impl Guest for Component {
    fn metadata() -> String {
        r#"{"module_id":"hevy","module_version":"1.0.0","source_api_version":"1.0.0","mapping_version":"1.0.0","provided_capabilities":["body.weight","body.fat_percentage"],"extension_contracts":["hevy.body-circumference@1.0.0"],"localization_namespace":"source.hevy"}"#.to_owned()
    }

    fn contract_version() -> String {
        "1.0.0".to_owned()
    }

    fn detect(asset: &AssetReader) -> u8 {
        let bytes = match read_asset(asset) {
            Ok(bytes) => bytes,
            Err(_) => return 0,
        };
        let mut memory = MemoryAsset { bytes };
        match crate::detect_hevy(&mut memory) {
            crate::ProbeResult::Match(crate::HevyArtifact::Measurements) => 100,
            _ => 0,
        }
    }

    fn validate(asset: &AssetReader) -> Result<String, String> {
        let metadata = asset.metadata();
        let input = crate::CsvInput::new(read_asset(asset)?, metadata.asset_id);
        let result = crate::context_for_measurements(&input)
            .and_then(|context| crate::parse_measurements(input, &context).map(|_| context));
        match result {
            Ok(context) => serde_json::to_string(&SourceValidation {
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
            .map_err(|error| error.to_string()),
            Err(error) => serde_json::to_string(&SourceValidation {
                valid: false,
                issues: vec![MappingIssue {
                    code: error.code().to_owned(),
                    message: error.detail(),
                    source_record_key: None,
                }],
                source_module_id: "hevy".to_owned(),
                source_api_version: "1.0.0"
                    .parse::<mfa_contracts::ContractVersion>()
                    .map_err(|error| error.to_string())?,
                logical_snapshot_key: "hevy:measurements:invalid".to_owned(),
                schema_fingerprint:
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_owned(),
                mapping_version: "1.0.0"
                    .parse::<mfa_contracts::ContractVersion>()
                    .map_err(|error| error.to_string())?,
            })
            .map_err(|error| error.to_string()),
        }
    }

    fn parse(asset: &AssetReader) -> Result<String, String> {
        let metadata = asset.metadata();
        let input = crate::CsvInput::new(read_asset(asset)?, metadata.asset_id);
        let context = crate::context_for_measurements(&input)
            .map_err(|error| format!("{}: {}", error.code(), error.detail()))?;
        let batch = crate::parse_measurements(input, &context)
            .map_err(|error| format!("{}: {}", error.code(), error.detail()))?;
        serde_json::to_string(&batch).map_err(|error| error.to_string())
    }
}

struct MemoryAsset {
    bytes: Vec<u8>,
}

impl crate::GuestAssetReader for MemoryAsset {
    fn read_all(&mut self) -> Result<Vec<u8>, String> {
        Ok(self.bytes.clone())
    }
}

fn read_asset(asset: &AssetReader) -> Result<Vec<u8>, String> {
    let metadata = asset.metadata();
    let byte_len = u32::try_from(metadata.byte_len)
        .map_err(|_| "asset exceeds guest reader u32 limit".to_owned())?;
    asset.read_at(0, byte_len)
}
