use std::fs;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::Arc;

use mako_core::anyhow::Result;

use crate::compiler::Context;
use crate::stats::StatsJsonMap;

pub struct Analyze {}

impl Analyze {
    pub fn write_analyze(stats: &StatsJsonMap, context: Arc<Context>) -> Result<()> {
        let stats_json = serde_json::to_string_pretty(&stats).unwrap();
        let html_str = format!(
            r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="UTF-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
    <link rel="stylesheet" type="text/css" href="index.css">
    <link rel="stylesheet" type="text/css" href="report.css">
  </head>
  <body>
    <div id="root"></div>
    <script>
      window.chartData = {};
    </script>
    <script src="./report.js"></script>
  </body>
</html>"#,
            stats_json
        );
        let report_path = context.config.output.path.join("report.html");
        fs::write(report_path, html_str).unwrap();
        // 获取项目根目录
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        // 构造 dist/index.js 文件的路径
        let index_file_path = project_root.join("../../client/dist/index.js");
        let index_file_css = project_root.join("../../client/dist/index.css");
        let file = File::open(index_file_path)?;
        let mut buf_reader = BufReader::new(file);

        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents)?;
        let report_path = context.config.output.path.join("report.js");

        fs::write(report_path, contents).unwrap();
        let file = File::open(index_file_css)?;
        let mut buf_reader = BufReader::new(file);

        let mut contents = String::new();
        buf_reader.read_to_string(&mut contents)?;
        let report_path = context.config.output.path.join("report.css");

        fs::write(report_path, contents).unwrap();
        Ok(())
    }
}
