import "./a_very_long_directory_name_that_must_not_appear_in_production_chunk_urls/initial.css";

globalThis.loadFeature = () =>
  import(
    "./a_very_long_directory_name_that_must_not_appear_in_production_chunk_urls/lazy.js"
  );
