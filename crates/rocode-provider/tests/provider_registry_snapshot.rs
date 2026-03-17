use rocode_provider::bootstrap::{
    bootstrap_config_from_raw, create_registry_from_bootstrap_config,
};
use std::collections::HashMap;

#[test]
fn test_provider_registry_snapshot() {
    // Create minimal bootstrap config to trigger provider registration
    let config = bootstrap_config_from_raw(HashMap::new(), vec![], vec![], None, None);
    let auth_store = HashMap::new();
    let registry = create_registry_from_bootstrap_config(&config, &auth_store);

    // Extract registered provider IDs
    let mut registered_ids: Vec<String> =
        registry.list().iter().map(|p| p.id().to_string()).collect();
    registered_ids.sort();

    // With no config, env, or auth store, the registry still exposes a small set
    // of credential-less/default providers (plus the custom-loader-backed opencode).
    //
    // This is a snapshot-style test: update this list only when the intended
    // default provider surface changes.
    assert_eq!(
        registered_ids,
        vec![
            "anthropic".to_string(),
            "google".to_string(),
            "minimax".to_string(),
            "minimax-cn".to_string(),
            "minimax-cn-coding-plan".to_string(),
            "minimax-coding-plan".to_string(),
            "opencode".to_string(),
        ]
    );
}
