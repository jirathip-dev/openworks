use openworks_core::{Call, Declaration, Fail, Mount, Reach, Row, Tx, Value, Word};
use openworks_db::{SELECTS, TABLES};

/// Writes the caller's whole company to the host's files as a zip, one CSV per table and every attached file; answers its key.
pub const CREATE: Declaration = Declaration::create("create", Word::Settings, SELECTS, create);

const ZIP_VERSION: u16 = 20;
const UTF8_NAMES: u16 = 0x0800;
const JAN_1980: u16 = 0x0021;

fn create(call: &Call, tx: &mut dyn Tx) -> Result<String, Fail> {
    let org = &call.who.org.0;
    let mut zip = Zip::default();
    for table in TABLES {
        let rows = tx.run(table.select, &[Value::Text(org.clone())])?;
        if table.name == "attachments" {
            for row in &rows {
                if let Some(Value::Text(id)) = row.first()
                    && let Some(bytes) = call.files.get(&format!("{org}/{id}"))?
                {
                    zip.add(&format!("files/{id}"), &bytes)?;
                }
            }
        }
        zip.add(&format!("{}.csv", table.name), csv(table.columns, &rows).as_bytes())?;
    }
    let key = format!("{org}/exports/{}.zip", (call.fresh)().0);
    call.files.put(&key, &zip.finish()?)?;
    Ok(key)
}

fn csv(columns: &[&str], rows: &[Row]) -> String {
    let mut out = columns.join(",");
    out.push_str("\r\n");
    for row in rows {
        let fields: Vec<String> = row.iter().map(field).collect();
        out.push_str(&fields.join(","));
        out.push_str("\r\n");
    }
    out
}

fn field(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Int(n) => n.to_string(),
        Value::Text(text) if text.contains([',', '"', '\r', '\n']) => format!("\"{}\"", text.replace('"', "\"\"")),
        Value::Text(text) => text.clone(),
    }
}

#[derive(Default)]
struct Zip {
    bytes: Vec<u8>,
    central: Vec<u8>,
    count: u16,
}

impl Zip {
    fn add(&mut self, name: &str, data: &[u8]) -> Result<(), Fail> {
        let offset = size(self.bytes.len())?;
        let length = size(data.len())?;
        let name_length = u16::try_from(name.len()).map_err(|_| too_large())?;
        self.count = self.count.checked_add(1).ok_or_else(too_large)?;
        let mut common = Vec::new();
        for half in [ZIP_VERSION, UTF8_NAMES, 0, 0, JAN_1980] {
            common.extend(half.to_le_bytes());
        }
        for word in [crc32(data), length, length] {
            common.extend(word.to_le_bytes());
        }
        common.extend(name_length.to_le_bytes());
        common.extend(0u16.to_le_bytes());
        self.bytes.extend(0x0403_4b50u32.to_le_bytes());
        self.bytes.extend(&common);
        self.bytes.extend(name.as_bytes());
        self.bytes.extend(data);
        self.central.extend(0x0201_4b50u32.to_le_bytes());
        self.central.extend(ZIP_VERSION.to_le_bytes());
        self.central.extend(&common);
        self.central.extend([0; 10]);
        self.central.extend(offset.to_le_bytes());
        self.central.extend(name.as_bytes());
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<u8>, Fail> {
        let start = size(self.bytes.len())?;
        let length = size(self.central.len())?;
        self.bytes.append(&mut self.central);
        self.bytes.extend(0x0605_4b50u32.to_le_bytes());
        self.bytes.extend([0; 4]);
        self.bytes.extend(self.count.to_le_bytes());
        self.bytes.extend(self.count.to_le_bytes());
        self.bytes.extend(length.to_le_bytes());
        self.bytes.extend(start.to_le_bytes());
        self.bytes.extend(0u16.to_le_bytes());
        Ok(self.bytes)
    }
}

fn size(length: usize) -> Result<u32, Fail> {
    u32::try_from(length).map_err(|_| too_large())
}

fn too_large() -> Fail {
    Fail::Invalid("the company is too large for one zip".to_owned())
}

fn crc32(data: &[u8]) -> u32 {
    !data.iter().fold(!0u32, |crc, &byte| {
        (0..8).fold(crc ^ u32::from(byte), |crc, _| {
            if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            }
        })
    })
}

pub(crate) fn mount() -> Mount {
    Mount::new("/exports", "exports").operation(&CREATE, Reach::Org)
}
