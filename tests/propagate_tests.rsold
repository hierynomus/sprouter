use kube::Client;
use k8s_openapi::api::core::v1::{ConfigMap, Namespace};
use kube::api::{Api, PostParams};
use std::collections::BTreeMap;
use rstest::rstest;

use shadower::propagate::*;
use shadower::utils::ANNOTATION_KEY;

fn annotated_cm(name: &str, ns: &str) -> ConfigMap {
    ConfigMap {
        metadata: kube::api::ObjectMeta {
            name: Some(name.into()),
            namespace: Some(ns.into()),
            annotations: Some(BTreeMap::from([(
                ANNOTATION_KEY.to_string(),
                "true".to_string(),
            )])),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[rstest]
#[tokio::test]
async fn test_propagate_all_to_namespace_skips_unannotated() {
    let client = Client::try_default().await.unwrap();

    let ns_api: Api<Namespace> = Api::all(client.clone());
    ns_api.create(&PostParams::default(), &Namespace {
        metadata: kube::api::ObjectMeta {
            name: Some("target-ns".to_string()),
            ..Default::default()
        },
        ..Default::default()
    }).await.unwrap();

    let cm_api: Api<ConfigMap> = Api::namespaced(client.clone(), "default");
    cm_api.create(&PostParams::default(), &ConfigMap {
        metadata: kube::api::ObjectMeta {
            name: Some("non-annotated".to_string()),
            namespace: Some("default".into()),
            ..Default::default()
        },
        ..Default::default()
    }).await.unwrap();

    propagate_all_to_namespace("target-ns", client.clone()).await.unwrap();

    let copied_api: Api<ConfigMap> = Api::namespaced(client.clone(), "target-ns");
    let result = copied_api.get_opt("non-annotated").await.unwrap();
    assert!(result.is_none()); // should not be propagated
}

#[rstest]
#[tokio::test]
async fn test_propagate_all_to_namespace_copies_annotated() {
    let client = Client::try_default().await.unwrap();

    let ns_api: Api<Namespace> = Api::all(client.clone());
    ns_api.create(&PostParams::default(), &Namespace {
        metadata: kube::api::ObjectMeta {
            name: Some("target-ns".to_string()),
            ..Default::default()
        },
        ..Default::default()
    }).await.unwrap();

    let cm = annotated_cm("annotated", "default");
    let cm_api: Api<ConfigMap> = Api::namespaced(client.clone(), "default");
    cm_api.create(&PostParams::default(), &cm).await.unwrap();

    propagate_all_to_namespace("target-ns", client.clone()).await.unwrap();

    let copied_api: Api<ConfigMap> = Api::namespaced(client.clone(), "target-ns");
    let result = copied_api.get_opt("annotated").await.unwrap();
    assert!(result.is_some()); // should be propagated
}
