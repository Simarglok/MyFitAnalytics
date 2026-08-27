wit_bindgen::generate!({
    path: "../../../../../modules/sdk/wit/source-api.wit",
    world: "source-module",
});

struct Component;

export!(Component);

impl Guest for Component {
    fn metadata() -> String {
        #[cfg(feature = "undeclared")]
        {
            r#"{"module_id":"guest-source","module_version":"1.0.0","source_api_version":"1.0.0","mapping_version":"1.0.0","provided_capabilities":["body.weight","secret.capability"],"extension_contracts":[],"localization_namespace":"source.guest"}"#.to_owned()
        }
        #[cfg(not(feature = "undeclared"))]
        {
            r#"{"module_id":"guest-source","module_version":"1.0.0","source_api_version":"1.0.0","mapping_version":"1.0.0","provided_capabilities":["body.weight"],"extension_contracts":[],"localization_namespace":"source.guest"}"#.to_owned()
        }
    }

    fn contract_version() -> String {
        "1.0.0".to_owned()
    }

    fn detect(asset: &AssetReader) -> u8 {
        match asset.read_at(0, 16) {
            Ok(bytes) if bytes.starts_with(b"ok") => 1,
            _ => 0,
        }
    }

    fn validate(asset: &AssetReader) -> Result<String, String> {
        let bytes = asset.read_at(0, 16).map_err(|error| error.to_string())?;
        if bytes.starts_with(b"malformed") {
            return Ok(r#"{"valid":true,"issues":[],"source_module_id":"guest-source","source_api_version":"1.0.0","logical_snapshot_key":"fixture:2026","schema_fingerprint":"sha256:0000000000000000000000000000000000000000000000000000000000000000","mapping_version":"1.0.0"}"#.to_owned());
        }
        Ok(r#"{"valid":true,"issues":[],"source_module_id":"guest-source","source_api_version":"1.0.0","logical_snapshot_key":"fixture:2026","schema_fingerprint":"sha256:0000000000000000000000000000000000000000000000000000000000000000","mapping_version":"1.0.0"}"#.to_owned())
    }

    fn parse(asset: &AssetReader) -> Result<String, String> {
        let bytes = asset.read_at(0, 16).map_err(|error| error.to_string())?;
        if bytes.starts_with(b"loop") {
            loop {
                core::hint::spin_loop();
            }
        }
        if bytes.starts_with(b"memory") {
            let mut memory = Vec::with_capacity(4 * 1024 * 1024);
            memory.resize(4 * 1024 * 1024, 7);
            core::hint::black_box(memory);
        }
        if bytes.starts_with(b"malformed") {
            return Ok("not-json".to_owned());
        }
        if bytes.starts_with(b"huge") {
            return Ok("x".repeat(128 * 1024));
        }
        if bytes.starts_with(b"invalid-record") {
            return Ok(r#"{"contract_version":"1.0.0","source_module_id":"guest-source","source_api_version":"1.0.0","mapping_version":"1.0.0","schema_fingerprint":"sha256:0000000000000000000000000000000000000000000000000000000000000000","logical_snapshot_key":"fixture:2026","source_records":[{"source_record_key":"duplicate","sheet_name":"fixture","source_row_number":1,"raw_payload":{}},{"source_record_key":"duplicate","sheet_name":"fixture","source_row_number":2,"raw_payload":{}}],"lineage":[],"records":[],"extensions":[],"issues":[]}"#.to_owned());
        }
        if bytes.starts_with(b"undeclared") {
            return Ok(r#"{"contract_version":"1.0.0","source_module_id":"guest-source","source_api_version":"1.0.0","mapping_version":"1.0.0","schema_fingerprint":"sha256:0000000000000000000000000000000000000000000000000000000000000000","logical_snapshot_key":"fixture:2026","source_records":[],"lineage":[],"records":[],"extensions":[{"namespace":"secret.capability","contract_version":"1.0.0","record_type":"output","source_record_key":"fixture:1","occurred_local_at":null,"local_date":null,"payload":{}}],"issues":[]}"#.to_owned());
        }
        Ok(r#"{"contract_version":"1.0.0","source_module_id":"guest-source","source_api_version":"1.0.0","mapping_version":"1.0.0","schema_fingerprint":"sha256:0000000000000000000000000000000000000000000000000000000000000000","logical_snapshot_key":"fixture:2026","source_records":[{"source_record_key":"fixture:1","sheet_name":"fixture","source_row_number":1,"raw_payload":{"raw":"ok"}}],"lineage":[{"canonical_entity_type":"body_measurement","canonical_entity_id":"00000000-0000-0000-0000-000000000001","source_record_key":"fixture:1","mapping_version":"1.0.0"}],"records":[{"type":"body_measurement","value":{"body_measurement_id":"00000000-0000-0000-0000-000000000001","local_date":"2026-08-25","weight_kg":82.5,"body_fat_pct":null,"source_record_id":"fixture:1"}}],"extensions":[],"issues":[]}"#.to_owned())
    }
}
