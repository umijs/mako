use std::fs;
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
    <style>{}</style>
  </head>
  <body>
    <div id="root"></div>
    <script>
      window.chartData = {};
    </script>
    <script>{}</script>
  </body>
</html>"#,
            include_str!("../../../../client/dist/index.css"),
            stats_json,
            include_str!("../../../../client/dist/index.js").replace("</script>", "<\\/script>")
        );
        let report_path = context.config.output.path.join("report.html");
        fs::write(report_path, html_str).unwrap();
        Ok(())
    }
}
