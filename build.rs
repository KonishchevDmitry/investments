use std::error::Error;
use std::path::Path;
use std::process;

fn generate() -> Result<(), Box<dyn Error + Send + Sync>> {
    let base_dir = Path::new("src/quotes/tinkoff");
    let protos_dir = base_dir.join("api/src/docs/contracts");

    let protos = [
        protos_dir.join("instruments.proto"),
        protos_dir.join("marketdata.proto"),
    ];
    let includes = [protos_dir];

    let out_dir = base_dir;
    let out_filename = "tinkoff.public.invest.api.contract.v1.rs";
    let out_path = out_dir.join(out_filename);

    // We don't want to depend on protoc in our crate, so generate the code only when it's missing
    // (we delete it during dependency update to force regeneration)
    if out_path.try_exists().map_err(|e| format!("Failed to stat() {:?}: {}", out_path, e))? {
        return Ok(());
    }

    let builder = tonic_build::configure()
        .build_server(false)
        .out_dir(out_dir);

    Ok(builder.compile_protos(&protos, &includes)?)
}

fn main() {
    if let Err(e) = generate() {
        eprintln!("Error: {}.", e.to_string().trim_end());
        process::exit(1);
    }
}