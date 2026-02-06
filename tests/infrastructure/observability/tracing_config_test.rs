use sandakan::infrastructure::observability::TracingConfig;

#[test]
fn given_no_env_vars_when_creating_default_then_uses_development() {
    let config = TracingConfig::default();
    assert!(!config.json_format);
}

#[test]
fn given_default_config_when_created_then_environment_is_set() {
    let config = TracingConfig::default();
    assert!(!config.environment.is_empty());
}
