use super::super::ConcatenateContext;
use crate::module::ModuleId;

pub fn describe_export_map(ccn_ctx: &ConcatenateContext) -> String {
    let map = ccn_ctx.modules_exports_map.get(&ModuleId::from("mut.js"));

    if let Some(export_map) = map {
        let mut keys = export_map.keys().collect::<Vec<&String>>();
        keys.sort();
        let mut describe = String::new();
        keys.into_iter().for_each(|key| {
            let (id, sub) = export_map.get(key).unwrap();

            if let Some(field) = sub {
                describe.push_str(&format!("{} => {}.{}\n", key, id.sym, field));
            } else {
                describe.push_str(&format!("{} => {}\n", key, id.sym));
            }
        });

        describe.trim().to_string()
    } else {
        "None".to_string()
    }
}
