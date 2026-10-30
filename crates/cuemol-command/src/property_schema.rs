use serde_json::{json, Value};

pub(super) fn sections(kind: &str) -> Value {
    if kind != "scene" {
        return json!([]);
    }
    json!([
        {"key":"scene-basic", "title":"Basic settings", "expanded":true, "rows":[
            {"key":"name", "label":"Name", "editable":true}
        ]},
        {"key":"scene-ao", "title":"Ambient occlusion", "expanded":true, "rows":[
            {"key":"aoEnabled", "label":"Enabled"},
            {"key":"ao-preset", "label":"Preset", "enabledWhen":"aoEnabled", "options":[
                {"label":"Low", "patch":{"aoRadius":"4", "aoSteps":"3", "aoIntensity":"2.2", "aoHalfRes":"false"}},
                {"label":"Medium", "patch":{"aoRadius":"8", "aoSteps":"4", "aoIntensity":"1.9", "aoHalfRes":"true"}},
                {"label":"High", "patch":{"aoRadius":"12", "aoSteps":"5", "aoIntensity":"1.7", "aoHalfRes":"true"}}
            ]}
        ]},
        {"key":"scene-aa", "title":"Anti-aliasing", "expanded":false, "rows":[
            {"key":"aa-preset", "label":"Quality", "options":[
                {"label":"Off", "patch":{"aa_method":"none", "aaJitterLevel":"0"}},
                {"label":"Standard (FXAA)", "patch":{"aa_method":"fxaa", "aaJitterLevel":"0"}},
                {"label":"High (FXAA + 8x SS)", "patch":{"aa_method":"fxaa", "aaJitterLevel":"3"}},
                {"label":"Ultra (FXAA + 32x SS)", "patch":{"aa_method":"fxaa", "aaJitterLevel":"5"}}
            ]}
        ]},
        {"key":"scene-bg", "title":"Background", "expanded":false, "rows":[
            {"key":"bgcolor", "label":"Color"}
        ]},
        {"key":"scene-proof", "title":"Color proofing", "expanded":false, "rows":[
            {"key":"use_colproof", "label":"Enabled", "seedWhenEnabled":{"icc_filename":"GenericCMYK.icm"}},
            {"key":"icc_filename", "label":"Profile"},
            {"key":"icc_intent", "label":"Intent", "labels":{
                "perceptual":"Perceptual", "relative_colorimetric":"Relative colorimetric",
                "saturation":"Saturation", "absolute_colorimetric":"Absolute colorimetric"
            }}
        ]}
    ])
}
