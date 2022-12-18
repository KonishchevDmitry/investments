use std::path::Path;
use std::process;

fn main() {
    let base_path = Path::new("src/quotes/tinkoff");
    let protos_path = base_path.join("api/src/docs/contracts");
    let protos = [protos_path.join("marketdata.proto")];
    let includes = [protos_path];

    let builder = tonic_build::configure()
        .build_server(false)
        .out_dir(base_path);

    if let Err(e) = builder.compile(&protos, &includes) {
        eprintln!("Error: {}.", e.to_string().trim_end());
        process::exit(1);
    }
}