#![forbid(unsafe_code)]

use rime_core::{Diagnostic, GraphQuantizationConfig, GraphRuntime, RuntimeSnapshot};
use rime_isp::{build_normal_graph_presentation, build_normal_manifest};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct NormalRuntime {
    runtime: GraphRuntime,
    manifest_json: String,
    quantization_config: GraphQuantizationConfig,
}

impl Default for NormalRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl NormalRuntime {
    #[wasm_bindgen(constructor)]
    #[must_use]
    ///
    /// # Panics
    ///
    /// Panics only if the statically generated Normal Graph quantization defaults are invalid.
    pub fn new() -> Self {
        let presentation = build_normal_graph_presentation();
        let quantization_config = GraphQuantizationConfig::defaults_for(&presentation)
            .expect("normal graph quantization defaults must be valid");
        let manifest_json = serde_json::to_string(&build_normal_manifest())
            .unwrap_or_else(|error| format!(r#"{{"serialization_error":"{error}"}}"#));
        Self {
            runtime: GraphRuntime::new(),
            manifest_json,
            quantization_config,
        }
    }


    /// Returns the validated graph quantization configuration.
    #[wasm_bindgen]
    #[must_use]
    pub fn quantization_config_json(&self) -> String {
        serde_json::to_string(&self.quantization_config)
            .unwrap_or_else(|error| format!(r#"{{"serialization_error":"{error}"}}"#))
    }
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn manifest_json(&self) -> String {
        self.manifest_json.clone()
    }

    /// Loads the Normal Graph.
    ///
    /// # Errors
    ///
    /// Returns a serialized lifecycle diagnostic when loading fails.
    pub fn load(&mut self) -> Result<String, JsValue> {
        self.runtime.begin_load().map_err(to_js_error)?;
        self.runtime.load_succeeded().map_err(to_js_error)?;
        snapshot_json(&self.runtime.snapshot())
    }

    /// Starts or replays the Normal Graph.
    ///
    /// # Errors
    ///
    /// Returns a serialized lifecycle diagnostic when running fails.
    pub fn run(&mut self) -> Result<String, JsValue> {
        self.runtime.run().map_err(to_js_error)?;
        snapshot_json(&self.runtime.snapshot())
    }

    /// Starts one visible-frame step.
    ///
    /// # Errors
    ///
    /// Returns a serialized lifecycle diagnostic when stepping fails.
    pub fn step(&mut self) -> Result<String, JsValue> {
        self.runtime.step().map_err(to_js_error)?;
        snapshot_json(&self.runtime.snapshot())
    }

    /// Commits the warmup phase.
    ///
    /// # Errors
    ///
    /// Returns a serialized lifecycle diagnostic when the transition fails.
    pub fn complete_warmup(&mut self) -> Result<String, JsValue> {
        self.runtime.complete_warmup().map_err(to_js_error)?;
        snapshot_json(&self.runtime.snapshot())
    }

    /// Commits the output phase.
    ///
    /// # Errors
    ///
    /// Returns a serialized lifecycle diagnostic when the transition fails.
    pub fn complete_output(&mut self) -> Result<String, JsValue> {
        self.runtime.complete_output().map_err(to_js_error)?;
        snapshot_json(&self.runtime.snapshot())
    }

    /// Resets the Normal Graph runtime.
    ///
    /// # Errors
    ///
    /// Returns a serialized lifecycle diagnostic when resetting fails.
    pub fn reset(&mut self) -> Result<String, JsValue> {
        self.runtime.reset().map_err(to_js_error)?;
        snapshot_json(&self.runtime.snapshot())
    }

    /// Invalidates output after a method or parameter update.
    ///
    /// # Errors
    ///
    /// Returns a serialized lifecycle diagnostic when the graph is executing.
    pub fn change_method(&mut self) -> Result<String, JsValue> {
        self.runtime.change_method().map_err(to_js_error)?;
        snapshot_json(&self.runtime.snapshot())
    }

    /// Validates and stores a graph quantization configuration.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for malformed JSON, invalid graph configuration, or an
    /// invalid lifecycle transition.
    pub fn set_quantization_config(&mut self, config_json: &str) -> Result<String, JsValue> {
        let config: GraphQuantizationConfig = serde_json::from_str(config_json)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        config
            .resolve(&build_normal_graph_presentation())
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.runtime.change_config().map_err(to_js_error)?;
        self.quantization_config = config;
        snapshot_json(&self.runtime.snapshot())
    }

    #[must_use]
    pub fn fail(&mut self) -> String {
        self.runtime.fail();
        serde_json::to_string(&self.runtime.snapshot())
            .unwrap_or_else(|error| format!(r#"{{"serialization_error":"{error}"}}"#))
    }

    #[must_use]
    pub fn device_lost(&mut self) -> String {
        self.runtime.device_lost();
        serde_json::to_string(&self.runtime.snapshot())
            .unwrap_or_else(|error| format!(r#"{{"serialization_error":"{error}"}}"#))
    }
}

fn snapshot_json(snapshot: &RuntimeSnapshot) -> Result<String, JsValue> {
    serde_json::to_string(snapshot).map_err(|error| JsValue::from_str(&error.to_string()))
}

fn to_js_error(diagnostic: Diagnostic) -> JsValue {
    match serde_json::to_value(diagnostic) {
        Ok(value) => JsValue::from_str(&value.to_string()),
        Err(error) => JsValue::from_str(&error.to_string()),
    }
}
