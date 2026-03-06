use crate::codegen::manifest::{GenerationMode, ResolvedConfig};
use crate::init_answers::InitAnswers;

pub(crate) fn render_versions_tf() -> String {
    let v = crate::terraform::REQUIRED_VERSION_CONSTRAINT;
    format!("terraform {{\n  required_version = \"{v}\"\n}}\n")
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
