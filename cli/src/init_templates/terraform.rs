use crate::codegen::manifest::{GenerationMode, ResolvedConfig};
use crate::config::schema::StateConfig;
use crate::init_answers::InitAnswers;

pub(crate) fn render_versions_tf(state: Option<&StateConfig>) -> String {
    let v = crate::terraform::REQUIRED_VERSION_CONSTRAINT;
    match state {
        Some(s) => {
            let backend = s.backend_type();
            format!("terraform {{\n  required_version = \"{v}\"\n\n  backend \"{backend}\" {{}}\n}}\n")
        }
        None => format!("terraform {{\n  required_version = \"{v}\"\n}}\n"),
    }
}

pub(crate) fn render_main_tf(answers: &InitAnswers) -> String {
    let resolved = ResolvedConfig::from_init_answers(answers);
    let module_source = crate::module_source();
    let module_body = crate::codegen::manifest::render_module_args(
        &resolved,
        GenerationMode::Power,
        &module_source,
    );

    format!("module \"evm_cloud\" {{\n{module_body}\n}}\n")
}

pub(crate) fn render_outputs_tf() -> String {
    "output \"workload_handoff\" {\n  value     = module.evm_cloud.workload_handoff\n  sensitive = true\n}\n"
        .to_string()
}

pub(crate) fn render_variables_tf(answers: &InitAnswers) -> String {
    let resolved = ResolvedConfig::from_init_answers(answers);
    crate::codegen::manifest::render_variables_tf(&resolved, GenerationMode::Power)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_versions_tf_no_state() {
        let output = render_versions_tf(None);
        assert!(output.contains("required_version"));
        assert!(!output.contains("backend"));
    }

    #[test]
    fn render_versions_tf_s3_empty_block() {
        let state = StateConfig::S3 {
            bucket: "b".to_string(),
            dynamodb_table: "t".to_string(),
            region: "r".to_string(),
            key: None,
            encrypt: true,
        };
        let output = render_versions_tf(Some(&state));
        assert!(output.contains("backend \"s3\" {}"));
        assert!(output.contains("required_version"));
        assert!(!output.contains("bucket"), "values should not be inline");
    }

    #[test]
    fn render_versions_tf_gcs_empty_block() {
        let state = StateConfig::Gcs {
            bucket: "b".to_string(),
            region: "r".to_string(),
            prefix: None,
        };
        let output = render_versions_tf(Some(&state));
        assert!(output.contains("backend \"gcs\" {}"));
    }
}
