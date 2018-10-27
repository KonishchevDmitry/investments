use std::fs::File;
use std::io::Read;

use serde_yaml;

use core::GenericResult;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(skip)]
    pub db_path: String,
}

pub fn load_config(path: &str) -> GenericResult<Config> {
    let mut data = Vec::new();
    File::open(path)?.read_to_end(&mut data)?;
    Ok(serde_yaml::from_slice(&data)?)
}