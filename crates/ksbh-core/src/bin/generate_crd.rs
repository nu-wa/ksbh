//! Simple binary to generate ModuleConfiguration CRD
//!
//! usage: generate_crd <output.yaml>

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use kube::CustomResourceExt;

    let output = std::env::args()
        .nth(1)
        .expect("usage: generate_crd <output.yaml>");

    let crd = ksbh_core::modules::ModuleConfiguration::crd();
    let yaml = serde_yaml_bw::to_string(&crd)?;

    std::fs::write(output, yaml)?;

    Ok(())
}
