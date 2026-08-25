wit_bindgen::generate!({
    path: "../../../../../modules/sdk/wit/dashboard-api.wit",
    world: "dashboard-module",
});

struct Component;

export!(Component);

impl Guest for Component {
    fn describe() -> String {
        r#"{"module_id":"guest-dashboard","module_version":"1.0.0","required_capabilities":["body.weight"],"localization_namespace":"dashboard.guest"}"#.to_owned()
    }

    fn compose(input_json: String) -> Result<String, String> {
        if input_json.contains("malformed") {
            return Ok("not-json".to_owned());
        }
        Ok(r#"{"title_key":"dashboard.guest.title","blocks":[{"type":"card","value":{"key":"weight","label":"dashboard.guest.weight","value":82.5}}]}"#.to_owned())
    }
}
