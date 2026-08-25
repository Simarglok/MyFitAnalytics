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
            r#"{"module_id":"guest-source","module_version":"1.0.0","provided_capabilities":["body.weight","secret.capability"],"localization_namespace":"source.guest"}"#.to_owned()
        }
        #[cfg(not(feature = "undeclared"))]
        {
            r#"{"module_id":"guest-source","module_version":"1.0.0","provided_capabilities":["body.weight"],"localization_namespace":"source.guest"}"#.to_owned()
        }
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
            return Ok(r#"{"valid":true,"issues":[]}"#.to_owned());
        }
        Ok(r#"{"valid":true,"issues":[]}"#.to_owned())
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
        if bytes.starts_with(b"undeclared") {
            return Ok(r#"{"records":[],"extensions":[{"namespace":"secret.capability","contract_version":"1.0.0","record_type":"output","payload":{}}],"issues":[]}"#.to_owned());
        }
        Ok(r#"{"records":[{"type":"body_measurement","value":{"body_measurement_id":"00000000-0000-0000-0000-000000000001","local_date":"2026-08-25","weight_kg":82.5,"body_fat_pct":null,"source_record_id":"fixture"}}],"extensions":[],"issues":[]}"#.to_owned())
    }
}
