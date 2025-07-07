use std::io::Read;

use encoding_rs::Encoding;
use serde::Deserialize;
use serde_xml_rs::Deserializer;
use xml::reader::{ParserConfig, EventReader, XmlEvent};

use crate::core::GenericResult;

// xml-rs supports only a very limited number of encodings, so handle them manually
pub fn deserialize<'de, R: Read, T: Deserialize<'de>>(mut reader: R) -> GenericResult<T> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let config = ParserConfig::new().ignore_invalid_encoding_declarations(true);
    let encoding_name = match EventReader::new_with_config(data.as_slice(), config.clone()).next()? {
        XmlEvent::StartDocument {encoding, ..} => encoding,
        _ => unreachable!(),
    };

    let encoding = Encoding::for_label(encoding_name.as_bytes()).ok_or_else(|| format!(
        "Unsupported XML document encoding: {encoding_name:?}"))?;

    let (data, _, errors) = encoding.decode(data.as_slice());
    if errors {
        return Err!("Got an invalid {} encoded data", encoding_name);
    }

    let config = config.override_encoding(Some(xml::Encoding::Utf8));
    let event_reader = EventReader::new_with_config(data.as_bytes(), config);
    let mut deserializer = Deserializer::new(event_reader);

    Ok(T::deserialize(&mut deserializer)?)
}