use std::fs;
use std::path::Path;

use anyhow::Result;

use crate::stats::StatsJsonMap;

pub struct Analyze {}

impl Analyze {
    pub fn write_analyze(stats: &StatsJsonMap, path: &Path) -> Result<()> {
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
        let report_path = path.join("analyze-report.html");
        fs::write(&report_path, html_str).unwrap();
        println!(
            "Analyze report generated at: {}",
            report_path.to_string_lossy()
        );
        Ok(())
    }
}
