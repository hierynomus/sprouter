use kube::api::ObjectMeta;

pub const ANNOTATION_KEY: &str = "shadower.geeko.me/enabled";

pub fn is_shadowed(meta: &ObjectMeta) -> bool {
    meta.annotations
        .as_ref()
        .and_then(|a| a.get(ANNOTATION_KEY))
        .map(|v| v == "true")
        .unwrap_or(false)
}
