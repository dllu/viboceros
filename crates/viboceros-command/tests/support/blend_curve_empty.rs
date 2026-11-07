//! Independent native document, identity, history and archive comparisons.
use super::{blend, measurements, points};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use viboceros_command::CommandRegistry;
use viboceros_document::{Document, ObjectId};

pub const FAMILIES: [&str; 2] = ["blend_curve_empty_discovery", "blend_curve_empty_holdouts"];
pub const CASES: usize = 27;

pub struct State {
    ids: BTreeMap<String, ObjectId>,
    jets: (f64, f64, usize),
}
impl State {
    pub fn capture(doc: &Document, value: &Value) -> Self {
        Self {
            ids: points::identities(doc),
            jets: blend::derivatives(doc, &value["before"]),
        }
    }
    pub fn verify(&self, doc: &mut Document, value: &Value) -> Value {
        let accepted = blend::accepted(value);
        assert_eq!(value["input_trace"][0]["errors"], json!([]));
        points::check_identities(doc, &self.ids, &value["after"]);
        let after_jets = blend::derivatives(doc, &value["after"]);
        let relative = self.jets.1.max(after_jets.1);
        assert!(relative <= 1e-6, "normalized derivatives: {relative:e}");
        let (samples, stations) = if accepted {
            blend::samples(doc, value)
        } else {
            (0., 0)
        };
        assert!(samples <= 1e-6, "Native samples: {samples:e}");
        let mut error = 0_f64;
        let mut domain = 0_f64;
        let mut ratio = 0_f64;
        let mut check = |doc: &Document, phase: &str, archive| {
            let (geometry, parameter) = blend::check_high(doc, &value[phase], archive);
            error = error.max(geometry);
            domain = domain.max(parameter);
            ratio = ratio.max(measurements::parameter_domain_ratio(doc, &value[phase]));
        };
        check(doc, "after", false);
        if accepted {
            assert_eq!(doc.undo_label(), Some("BlendCrv"));
            doc.undo().unwrap();
            check(doc, "undo", false);
            points::check_identities(doc, &self.ids, &value["before"]);
            assert!(!doc.can_undo());
            doc.redo().unwrap();
            check(doc, "redo", false);
            points::check_identities(doc, &self.ids, &value["after"]);
            assert!(!doc.can_redo());
        } else {
            assert!(!doc.can_undo());
            assert!(doc.can_redo());
            doc.redo().unwrap();
            check(doc, "pending_redo", false);
            doc.undo().unwrap();
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("blend.3dm");
        let registry = CommandRegistry::with_builtins();
        registry
            .execute(doc, &format!("SaveAs {}", path.display()))
            .unwrap();
        let mut restored = Document::new(doc.tolerance());
        registry
            .execute(&mut restored, &format!("Open3dm {}", path.display()))
            .unwrap();
        check(&restored, "after", true);
        assert!(error <= 1e-6, "coefficients/samples: {error:e}");
        assert!(
            ratio <= 1.,
            "parameter domain error: {domain:e}, ratio: {ratio:e}"
        );
        json!({"maximum_coefficient_and_sample_error":error,
            "maximum_native_sample_error":samples,"native_stations":stations,
            "maximum_parameter_domain_error":domain,"maximum_parameter_domain_gate_ratio":ratio,
            "maximum_normalized_derivative_error":relative,
            "maximum_absolute_derivative_error":self.jets.0.max(after_jets.0),
            "derivative_stations":self.jets.2+after_jets.2,"archives":1,"accepted":accepted})
    }
}
