//! The part of each tool's argument schema the model sees on every turn. The server still accepts every argument;
//! the ones left out here were never used by an agent in the audited runs (`docs/audits/token-audit.md`), and the
//! hints that need one name it.

use serde_json::{Map, Value};

const HIDDEN: &[(&str, &[&str])] = &[
    (
        "snapshot",
        &["diff", "interactive", "within", "max_lines", "pixels"],
    ),
    ("tap", &["role", "count", "long"]),
    ("type", &["mode", "append"]),
    ("swipe", &["distance", "fast"]),
    ("scroll", &["in", "max"]),
    ("shell", &["grep", "head", "settle", "force", "secret"]),
    ("batch", &["timeout_ms"]),
];

pub fn lean(tool: &str, schema: &mut Map<String, Value>) {
    let Some(Value::Object(props)) = schema.get_mut("properties") else {
        return;
    };
    props.remove("device");
    for (name, hidden) in HIDDEN {
        if *name == tool {
            for h in *hidden {
                props.remove(*h);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn schema() -> Map<String, Value> {
        json!({"type":"object","properties":{
            "device":{"type":"string"},"target":{"type":"string"},"role":{"type":"string"},"long":{"type":"integer"}
        },"required":["target"]})
        .as_object()
        .unwrap()
        .clone()
    }

    #[test]
    fn a_tool_loses_the_device_argument_and_its_unused_arguments_only() {
        let mut s = schema();
        lean("tap", &mut s);
        let props = s["properties"].as_object().unwrap();
        assert_eq!(props.keys().collect::<Vec<_>>(), ["target"]);
        assert_eq!(s["required"], json!(["target"]));
    }

    #[test]
    fn a_tool_with_nothing_listed_keeps_everything_but_the_device() {
        let mut s = schema();
        lean("wait", &mut s);
        let props = s["properties"].as_object().unwrap();
        assert_eq!(props.len(), 3);
        assert!(!props.contains_key("device"));
    }
}
