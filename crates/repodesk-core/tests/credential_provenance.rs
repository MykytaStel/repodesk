use repodesk_core::credentials::{
    CredentialResolver, CredentialSource, OPENAI_API_KEY, effective_credential_metadata,
};
use repodesk_core::errors::RepoDeskResult;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryResolver(Mutex<HashMap<String, String>>);

impl MemoryResolver {
    fn with(key: &str, value: &str) -> Self {
        let resolver = Self::default();
        resolver.set(key, value).expect("seed credential");
        resolver
    }
}

impl CredentialResolver for MemoryResolver {
    fn get(&self, key: &str) -> RepoDeskResult<Option<String>> {
        Ok(self.0.lock().expect("resolver lock").get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> RepoDeskResult<()> {
        self.0
            .lock()
            .expect("resolver lock")
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> RepoDeskResult<()> {
        self.0.lock().expect("resolver lock").remove(key);
        Ok(())
    }
}

#[test]
fn effective_metadata_prefers_keychain_without_exposing_secret() {
    let keychain = MemoryResolver::with(OPENAI_API_KEY, "fixture-aaaa");
    let environment = MemoryResolver::with(OPENAI_API_KEY, "fixture-bbbb");

    let metadata = effective_credential_metadata(&keychain, &environment, OPENAI_API_KEY)
        .expect("metadata resolves");

    assert!(metadata.configured);
    assert_eq!(metadata.source, CredentialSource::Keychain);
    assert_eq!(metadata.hint, "••••aaaa");
    assert!(!format!("{metadata:?}").contains("fixture-aaaa"));
    assert!(!format!("{metadata:?}").contains("fixture-bbbb"));
}
